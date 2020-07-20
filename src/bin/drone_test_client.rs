// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezedge_debugger::{
    utility::stream::MessageStream,
    utility::identity::Identity,
};
use lazy_static::lazy_static;
use structopt::StructOpt;
use tokio::net::TcpStream;
use crypto::nonce::{Nonce, NoncePair, generate_nonces};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryMessage};
use crypto::crypto_box::precompute;
use tezedge_debugger::utility::stream::{EncryptedMessageWriter, EncryptedMessageReader};
use tezos_messages::p2p::encoding::peer::{PeerMessageResponse, PeerMessage};
use tezos_messages::p2p::encoding::advertise::AdvertiseMessage;
use std::net::{SocketAddr, IpAddr};
use std::convert::TryFrom;
use tezos_messages::p2p::binary_message::cache::CachedData;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use tezos_messages::p2p::encoding::ack::AckMessage;

lazy_static! {
    static ref IDENTITY: Identity = Identity {
        peer_id: "idrj5eYTN6BgrzCT1YQh3mCVuWciVr".to_string(),
        public_key: "df06423ed30c9777b0089a8de406ffa10988bb0655b4a9e4c814fe326ee0f33b".to_string(),
        secret_key: "c60b4be2c6a1d25f58e6abd70847a94cc922c16c689b7a2ba9d567af2ccdec06".to_string(),
        proof_of_work_stamp: "74ed18aa2c733e0cbde54e2e7fb9dab28665a3a4d3a9cb08".to_string(),
    };

    static ref NONCE: Nonce = Nonce::random();
}

#[derive(StructOpt, Debug)]
#[structopt(name = "drone testing client")]
struct Opt {
    #[structopt(short, long, default_value = "1")]
    pub clients: u32,
    #[structopt(short, long, default_value = "1")]
    pub messages: u32,
    #[structopt(short, long, default_value = "0.0.0.0:13030")]
    pub server: String,
}

async fn test_client(id: u32, messages: u32, server: String) {
    println!("[{}] Running test client against \"{}\"", id, server);
    let stream = TcpStream::connect(server).await
        .expect("failed to connect to test server");
    println!("[{}] Connected to server", id);

    let (mut reader, mut writer) = MessageStream::from(stream).split();
    let sent_conn_msg = ConnectionMessage::new(
        0,
        &IDENTITY.public_key,
        &IDENTITY.proof_of_work_stamp,
        &NONCE.get_bytes(),
        Default::default(),
    );
    let chunk = BinaryChunk::from_content(&sent_conn_msg.as_bytes().unwrap()).unwrap();

    writer.write_message(&chunk).await
        .unwrap();
    let recv_chunk = reader.read_message().await
        .unwrap();
    println!("[{}] Received connection message", id);
    let recv_conn_msg = ConnectionMessage::try_from(recv_chunk)
        .expect("got invalid connection message from server");

    let sent_data = chunk;
    let recv_data = BinaryChunk::from_content(&recv_conn_msg.cache_reader().get().unwrap()).unwrap();

    let precomputed_key = precompute(
        &hex::encode(recv_conn_msg.public_key),
        &IDENTITY.secret_key,
    ).unwrap();

    let NoncePair { remote, local } = generate_nonces(
        sent_data.raw(),
        recv_data.raw(),
        true,
    );

    let mut writer = EncryptedMessageWriter::new(writer, precomputed_key.clone(), remote, IDENTITY.peer_id.clone());
    let mut reader = EncryptedMessageReader::new(reader, precomputed_key.clone(), local, IDENTITY.peer_id.clone());

    println!("[{}] Encrypted connection", id);

    let sent_metadata = MetadataMessage::new(true, true);
    writer.write_message(&sent_metadata).await.unwrap();
    println!("[{}] Sent metadata message", id);
    let recv_metadata = reader.read_message::<MetadataMessage>()
        .await.unwrap();
    assert_eq!(sent_metadata.as_bytes(), recv_metadata.as_bytes(), "received different metadata");
    println!("[{}] Got metadata message", id);

    let sent_ack = AckMessage::Ack;
    writer.write_message(&sent_ack).await.unwrap();
    let recv_ack = reader.read_message::<AckMessage>()
        .await.unwrap();
    assert_eq!(sent_ack, recv_ack, "received different acks");
    println!("[{}] Got Ack message", id);

    for msg_id in 0..messages {
        let message = PeerMessage::Advertise(AdvertiseMessage::new(&[
            SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0)
        ]));
        let message = PeerMessageResponse::from(message);
        writer.write_message(&message).await
            .unwrap();
        println!("[{}] Sent encrypted message {}", id, msg_id);
        let recv_message = reader.read_message::<PeerMessageResponse>().await
            .unwrap();
        assert_eq!(message.as_bytes(), recv_message.as_bytes(), "Received different message");
        println!("[{}] Got re-encrypted message {} back", id, msg_id);
    }
}

#[tokio::main]
pub async fn main() -> std::io::Result<()> {
    let opts: Opt = Opt::from_args();
    let mut handles = Vec::with_capacity(opts.clients as usize);
    for id in 0..opts.clients {
        handles.push(tokio::spawn(test_client(id, opts.messages, opts.server.clone())));
    }

    for handle in handles {
        handle.await?;
    }

    Ok(())
}