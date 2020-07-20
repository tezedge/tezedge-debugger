use std::net::{SocketAddr, IpAddr};
use tokio::net::{TcpListener, TcpStream};
use lazy_static::lazy_static;
use crate::utility::identity::Identity;
use crate::utility::stream::{MessageStream, EncryptedMessageWriter, EncryptedMessageReader};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use crypto::nonce::{Nonce, NoncePair, generate_nonces};
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryMessage};
use std::convert::TryFrom;
use tezos_messages::p2p::binary_message::cache::CachedData;
use crypto::crypto_box::precompute;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use tezos_messages::p2p::encoding::ack::AckMessage;
use tezos_messages::p2p::encoding::advertise::AdvertiseMessage;
use tezos_messages::p2p::encoding::prelude::{PeerMessageResponse, PeerMessage as TezosPeerMessage};
use crate::messages::prelude::PeerMessage;
use tracing::{error, warn, info, field::{debug, display}};
use crate::messages::p2p_message::P2pMessage;
use tezos_messages::p2p::encoding::version::Version;

lazy_static! {
    static ref IDENTITY: Identity = Identity {
        peer_id: "idsscFHxXoeJjxQsQBeEveayLyvymA".to_string(),
        public_key: "b41df26473332e7225fdad07045112b5ba6bf295a384785c535cf738575ee245".to_string(),
        secret_key: "dc9640dbd8cf50a5475b6a6d65c96af943380a627cea198906a2a8d4fd37decc".to_string(),
        proof_of_work_stamp: "d0e1945cb693c743e82b3e29750ebbc746c14dbc280c6ee6".to_string(),
    };
}

#[allow(dead_code)]
pub async fn replay(node_address: SocketAddr, messages: Vec<P2pMessage>, incoming: bool) -> Result<(), failure::Error> {
    warn!(message_count = messages.len(), incoming, "starting replay of messages");
    if incoming {
        replay_incoming(node_address, messages).await
    } else {
        replay_outgoing(node_address, messages).await
    }
}

#[allow(dead_code)]
async fn replay_incoming(node_address: SocketAddr, messages: Vec<P2pMessage>) -> Result<(), failure::Error> {
    tokio::spawn(async move {
        let err: Result<(), failure::Error> = async move {
            let stream = TcpStream::connect(node_address).await?;
            let (reader, writer) = MessageStream::from(stream).split();
            let (mut reader, mut writer) = (Some(reader), Some(writer));
            let (mut enc_reader, mut enc_writer): (Option<EncryptedMessageReader>, Option<EncryptedMessageWriter>) = (None, None);
            let messages = messages.into_iter();
            let mut encrypted = false;
            let mut metadata_count: usize = 0;
            let mut ack_count: usize = 0;
            let mut received_connection_message: Option<ConnectionMessage> = None;
            let mut sent_connection_message: Option<ConnectionMessage> = None;

            for mut message in messages.rev() {
                if !encrypted && sent_connection_message.is_some() && received_connection_message.is_some() {
                    let sent = sent_connection_message.clone().unwrap();
                    let recv = received_connection_message.clone().unwrap();
                    let sent_data = BinaryChunk::from_content(&sent.as_bytes()?)?;
                    let recv_data = BinaryChunk::from_content(&recv.as_bytes()?)?;
                    let pk = precompute(
                        &hex::encode(&recv.public_key),
                        &IDENTITY.secret_key,
                    )?;
                    let NoncePair { remote, local } = generate_nonces(
                        sent_data.raw(),
                        recv_data.raw(),
                        false,
                    );
                    let writer = std::mem::replace(&mut writer, None);
                    let reader = std::mem::replace(&mut reader, None);
                    enc_writer = Some(EncryptedMessageWriter::new(writer.unwrap(), pk.clone(), local, IDENTITY.peer_id.clone()));
                    enc_reader = Some(EncryptedMessageReader::new(reader.unwrap(), pk.clone(), remote, IDENTITY.peer_id.clone()));
                    encrypted = true;
                }
                let sending = !message.incoming;
                if message.message.len() < 1 {
                    continue;
                }
                let message = message.message.pop().unwrap();
                info!(sending, encrypted, "Processing message");
                if sending {
                    if encrypted {
                        info!(msg = debug(&message), "Sending encrypted message message");
                        match message {
                            PeerMessage::ConnectionMessage(_) => return Ok(()),
                            PeerMessage::AckMessage(ack) => {
                                ack_count += 1;
                                enc_writer.as_mut().unwrap().write_message(&ack).await?;
                            }
                            PeerMessage::MetadataMessage(metadata) => {
                                metadata_count += 1;
                                enc_writer.as_mut().unwrap().write_message(&metadata).await?;
                            }
                            message => {
                                let message = PeerMessageResponse::from(message.inner().unwrap().clone());
                                enc_writer.as_mut().unwrap().write_message(&message).await?;
                            }
                        }
                    } else {
                        match message {
                            PeerMessage::ConnectionMessage(mut conn_msg) => {
                                conn_msg.public_key = hex::decode(&IDENTITY.public_key)?;
                                conn_msg.versions.push(Version::new("TEZOS_ALPHANET_CARTHAGE_2019-11-28T13:02:13Z".to_string(), 0, 1));
                                let sent_chunk = BinaryChunk::from_content(&conn_msg.as_bytes()?)?;
                                sent_connection_message = Some(conn_msg);
                                info!(msg = debug(&sent_connection_message), "Sending connection message");
                                writer.as_mut().unwrap().write_message(&sent_chunk)
                                    .await.unwrap();
                            }
                            _ => return Ok(()),
                        }
                    }
                } else {
                    if encrypted {
                        let reader = enc_reader.as_mut().unwrap();
                        if metadata_count < 2 {
                            let msg = reader.read_message::<MetadataMessage>().await?;
                            metadata_count += 1;
                            info!(msg = debug(&msg), "Received metadata message");
                        } else if ack_count < 2 {
                            let msg = reader.read_message::<AckMessage>().await?;
                            ack_count += 1;
                            info!(msg = debug(&msg), "Received ack message");
                        } else {
                            let msg = reader.read_message::<PeerMessageResponse>().await?;
                            info!(msg = debug(&msg), "Received encrypted message");
                        }
                    } else {
                        let recv_chunk = reader.as_mut().unwrap().read_message().await?;
                        let conn_msg = ConnectionMessage::try_from(recv_chunk)?;
                        received_connection_message = Some(conn_msg);
                        info!(msg = debug(&received_connection_message), "Received connection message");
                    }
                }
            }
            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            Ok(())
        }.await;
        if let Err(err) = err {
            error!(err = display(&err), "failed to replay");
        }
    });
    Ok(())
}

#[allow(dead_code)]
async fn replay_outgoing(node_address: SocketAddr, messages: Vec<P2pMessage>) -> Result<(), failure::Error> {
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let listening_port = listener.local_addr()?.port();
    // Spawn replay handler
    tokio::spawn(async move {
        let err: Result<(), failure::Error> = async move {
            let mut listener = listener;
            let (stream, _peer_addr) = listener.accept().await?;
            let (reader, writer) = MessageStream::from(stream).split();
            let (mut reader, mut writer) = (Some(reader), Some(writer));
            let (mut enc_reader, mut enc_writer): (Option<EncryptedMessageReader>, Option<EncryptedMessageWriter>) = (None, None);
            let messages = messages.into_iter();
            let mut encrypted = false;
            let mut metadata_count: usize = 0;
            let mut ack_count: usize = 0;
            let mut received_connection_message: Option<ConnectionMessage> = None;
            let mut sent_connection_message: Option<ConnectionMessage> = None;

            for mut message in messages.rev() {
                if !encrypted && sent_connection_message.is_some() && received_connection_message.is_some() {
                    let sent = sent_connection_message.clone().unwrap();
                    let recv = received_connection_message.clone().unwrap();
                    let sent_data = BinaryChunk::from_content(&sent.as_bytes()?)?;
                    let recv_data = BinaryChunk::from_content(&recv.as_bytes()?)?;
                    let pk = precompute(
                        &hex::encode(&recv.public_key),
                        &IDENTITY.secret_key,
                    )?;
                    let NoncePair { remote, local } = generate_nonces(
                        sent_data.raw(),
                        recv_data.raw(),
                        true,
                    );
                    let writer = std::mem::replace(&mut writer, None);
                    let reader = std::mem::replace(&mut reader, None);
                    enc_writer = Some(EncryptedMessageWriter::new(writer.unwrap(), pk.clone(), remote, IDENTITY.peer_id.clone()));
                    enc_reader = Some(EncryptedMessageReader::new(reader.unwrap(), pk.clone(), local, IDENTITY.peer_id.clone()));
                    encrypted = true;
                }

                let sending = !message.incoming;
                if message.message.len() < 1 {
                    continue;
                }
                let message = message.message.pop().unwrap();

                if sending {
                    if encrypted {
                        match message {
                            PeerMessage::ConnectionMessage(_) => return Ok(()),
                            PeerMessage::MetadataMessage(metadata) => {
                                metadata_count += 1;
                                enc_writer.as_mut().unwrap().write_message(&metadata).await?;
                            }
                            message => {
                                let message = PeerMessageResponse::from(message.inner().unwrap().clone());
                                enc_writer.as_mut().unwrap().write_message(&message).await?;
                            }
                        }
                    } else {
                        match message {
                            PeerMessage::ConnectionMessage(mut conn_msg) => {
                                conn_msg.public_key = hex::decode(&IDENTITY.public_key)?;
                                conn_msg.versions.push(Version::new("TEZOS_ALPHANET_CARTHAGE_2019-11-28T13:02:13Z".to_string(), 0, 1));
                                let sent_chunk = BinaryChunk::from_content(&conn_msg.as_bytes()?)?;
                                sent_connection_message = Some(conn_msg);
                                writer.as_mut().unwrap().write_message(&sent_chunk)
                                    .await.unwrap();
                            }
                            _ => return Ok(()),
                        }
                    }
                } else {
                    if encrypted {
                        let reader = enc_reader.as_mut().unwrap();
                        if metadata_count < 2 {
                            let _ = reader.read_message::<MetadataMessage>().await?;
                            metadata_count += 1;
                        // } else if ack_count < 2 {
                        //     let _ = reader.read_messge::<AckMessage>().await?;
                        //     ack_count += 1;
                        } else {
                            let _ = reader.read_message::<PeerMessageResponse>().await?;
                        }
                    } else {
                        let recv_chunk = reader.as_mut().unwrap().read_message().await?;
                        let conn_msg = ConnectionMessage::try_from(recv_chunk)?;
                        received_connection_message = Some(conn_msg);
                    }
                }
            }
            Ok(())
        }.await;
        if let Err(err) = err {
            error!(err = display(&err), "failed to replay");
        }
    });

    let connection = TcpStream::connect(node_address).await?;
    let (mut reader, mut writer) = MessageStream::from(connection).split();
    let sent_conn_msg = ConnectionMessage::new(
        0,
        &IDENTITY.public_key,
        &IDENTITY.proof_of_work_stamp,
        &Nonce::random().get_bytes(),
        Default::default(),
    );
    let chunk = BinaryChunk::from_content(&sent_conn_msg.as_bytes().unwrap()).unwrap();

    writer.write_message(&chunk).await?;
    let recv_chunk = reader.read_message().await?;
    let recv_conn_msg = ConnectionMessage::try_from(recv_chunk)?;
    let sent_data = chunk;
    let recv_data = BinaryChunk::from_content(&recv_conn_msg.cache_reader().get().unwrap_or_default())?;
    let pk = precompute(
        &hex::encode(recv_conn_msg.public_key),
        &IDENTITY.secret_key,
    )?;

    let NoncePair { remote, local } = generate_nonces(
        sent_data.raw(),
        recv_data.raw(),
        false,
    );

    let mut writer = EncryptedMessageWriter::new(writer, pk.clone(), local, IDENTITY.peer_id.clone());
    let mut reader = EncryptedMessageReader::new(reader, pk.clone(), remote, IDENTITY.peer_id.clone());
    let sent_metadata = MetadataMessage::new(true, false);
    writer.write_message(&sent_metadata).await?;
    reader.read_message::<MetadataMessage>().await?;
    writer.write_message(&AckMessage::Ack).await?;
    reader.read_message::<AckMessage>().await?;
    let advertise = TezosPeerMessage::Advertise(AdvertiseMessage::new(&[
        SocketAddr::new(IpAddr::from([0, 0, 0, 0]), listening_port),
    ]));
    let message = PeerMessageResponse::from(advertise);
    writer.write_message(&message).await?;

    Ok(())
}