use std::{net::SocketAddr, iter::ExactSizeIterator, convert::TryFrom, io};
use tezos_messages::p2p::binary_message::{BinaryMessage, BinaryChunk};
use tezos_conversation::{Decipher, Identity, NonceAddition};
use tokio::{net::TcpStream, io::{AsyncReadExt, AsyncWriteExt}};
use bytes::Buf;
use crate::messages::p2p_message::P2pMessage;

/// Create an replay of given message onto the given address
pub async fn replay<I>(node_address: SocketAddr, messages: I) -> Result<(), failure::Error>
where
    I: Iterator<Item = P2pMessage> + ExactSizeIterator + Send + 'static,
{
    let mut messages = messages;

    // constant identity
    let identity = Identity::from_json("\
        {\
            \"peer_id\":\"idtJunqYgD1M6r6o2qvGpiD5xKZWRu\",\
            \"public_key\":\"7e8108e598b056b52cb430ee0e5e7ffd080b1b6bd9c9ad17dd9c44e2ced7fd75\",\
            \"secret_key\":\"a9f36be41dd4cfec7ec1e4a134d660254006e0ce16ae272f10dbc19a3097adcf\",\
            \"proof_of_work_stamp\":\"79eb7e72262e067a7e4e65fedacef484be52a35de686d1c8\"\
        }\
    ").unwrap();

    // TODO: error handling
    let init_connection_message = messages.next().unwrap();

    let incoming = init_connection_message.incoming;
    tracing::info!(message_count = messages.len(), incoming, "starting replay of messages");
    if incoming {
        let mut icm = init_connection_message;
        let icm = icm.message.first_mut().unwrap().as_mut_cm().unwrap();
        icm.public_key = identity.public_key();
        let icm_chunk = BinaryChunk::from_content(&icm.as_bytes()?)?;
        let mut stream = TcpStream::connect(node_address).await?;
        stream.write_all(icm_chunk.raw()).await?;
        let rcm_chunk = read_chunk(&mut stream).await?;
        let decipher = identity.decipher(icm_chunk.raw(), rcm_chunk.raw()).ok().unwrap();
        replay_messages(stream, messages, decipher, true).await
    } else {
        // TODO:
        Ok(())
    }
}

pub async fn read_chunk<R>(stream: &mut R) -> Result<BinaryChunk, io::Error>
where
    R: Unpin + AsyncReadExt,
{
    let mut chunk_buffer = vec![0; 0x10000];
    stream.read_exact(&mut chunk_buffer[0..2]).await?;
    let size = (&chunk_buffer[0..2]).get_u16() as usize;
    stream.read_exact(&mut chunk_buffer[2..(2 + size)]).await?;

    Ok(BinaryChunk::try_from(chunk_buffer).unwrap())
}

async fn replay_messages<I>(
    stream: TcpStream,
    messages: I,
    decipher: Decipher,
    incoming: bool,
) -> Result<(), failure::Error>
where
    I: Iterator<Item = P2pMessage> + Send + 'static,
{
    tokio::spawn(async move {
        let err: Result<(), failure::Error> = async move {
            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            let mut initiators = 0;
            let mut responders = 0;
            let mut stream = stream;
            for message in messages {
                // if this message is incoming and first message is also incoming
                // or this message is outgoing and first message is also outgoing
                // then the message goes from initiator
                let chunk_number = if message.incoming == incoming {
                    let a = NonceAddition::Initiator(initiators);
                    initiators += 1;
                    a
                } else {
                    let a = NonceAddition::Responder(responders);
                    responders += 1;
                    a
                };
                if message.incoming {
                    let mut bytes = message.decrypted_bytes;
                    let l = bytes.len() - 16; // cut mac
                    let encrypted = decipher.encrypt(&mut bytes[..l], chunk_number).unwrap();
                    let chunk = BinaryChunk::try_from(encrypted).unwrap();
                    stream.write_all(chunk.raw()).await?;
                } else {
                    let chunk = read_chunk(&mut stream).await?;
                    let decrypted = decipher.decrypt(chunk.content(), chunk_number).unwrap();
                    if decrypted != message.decrypted_bytes {
                        tracing::error!("unexpected chunk");
                    }
                }
            }
            Ok(())
        }.await;
        if let Err(err) = err {
            tracing::error!(err = tracing::field::display(&err), "failed to replay");
        }
    });
    Ok(())
}
