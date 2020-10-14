use std::{net::SocketAddr, iter::ExactSizeIterator};
// use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use crate::messages::p2p_message::P2pMessage;

/// Create an replay of given message onto the given address
pub async fn replay<I>(node_address: SocketAddr, messages: I) -> Result<(), failure::Error>
where
    I: Iterator<Item = P2pMessage> + ExactSizeIterator + Send + 'static,
{
    let mut messages = messages;

    // TODO: error handling
    let init_connection_message = messages.next().unwrap();
    let _icm = init_connection_message.message.first().unwrap().as_connection_message().unwrap();
    let resp_connection_message = messages.next().unwrap();
    let _rcm = resp_connection_message.message.first().unwrap().as_connection_message().unwrap();

    // TODO: create keys

    let incoming = init_connection_message.incoming;
    tracing::info!(message_count = messages.len(), incoming, "starting replay of messages");
    if incoming {
        replay_incoming(node_address, messages).await
    } else {
        // replay_outgoing(node_address, messages).await
        Ok(())
    }
}

/// Replay given messages to the given address as if this replay is an actual node driven by
/// the given message
async fn replay_incoming<I>(node_address: SocketAddr, messages: I) -> Result<(), failure::Error>
where
    I: Iterator<Item = P2pMessage> + Send + 'static,
{
    tokio::spawn(async move {
        let err: Result<(), failure::Error> = async move {
            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            let _ = node_address;
            for message in messages {
                // TODO: reencrypt and send
                let _ = message;
            }
            Ok(())
        }.await;
        if let Err(err) = err {
            tracing::error!(err = tracing::field::display(&err), "failed to replay");
        }
    });
    Ok(())
}
