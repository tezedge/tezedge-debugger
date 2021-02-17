use std::{net::SocketAddr, mem};
use futures::future::Either;
use tokio::sync::mpsc::{self, error::SendError};

use super::p2p;
use crate::messages::p2p_message::SourceType;

pub struct Connection {
    state: ConnectionState,
    tx: mpsc::UnboundedSender<Either<p2p::Message, p2p::Command>>,
    source_type: SourceType,
    // it is possible we receive/send connection message in wrong order
    // do connect and receive the message and then send
    // or do accept and send the message and then receive
    // probably it is due to TCP Fast Open
    remote_address: SocketAddr,
}

enum ConnectionState {
    Initial,
    Completed,
    CorrectOrder,
    Unordered(p2p::Message),
    Invalid,
}

impl Connection {
    pub fn new(
        tx: mpsc::UnboundedSender<Either<p2p::Message, p2p::Command>>,
        source_type: SourceType,
        remote_address: SocketAddr,
    ) -> Self {
        Connection {
            state: ConnectionState::Initial,
            tx,
            source_type,
            remote_address,
        }
    }

    pub fn process(&mut self, message: p2p::Message) {
        let local_is_initiator = self.source_type.is_local();
        let state = mem::replace(&mut self.state, ConnectionState::Invalid);
        let state = match state {
            // the state is initial, initiator is local, but message is incoming, or vice versa
            // it means the connection messages are in wrong order
            ConnectionState::Initial if local_is_initiator == message.incoming => {
                // strange 24 bytes message, it is not a connection message,
                // let's ignore it and write in log
                if message.payload.len() == 24 && message.incoming {
                    tracing::error!(
                        id = tracing::field::display(&message.event_id),
                        payload = tracing::field::display(hex::encode(message.payload.as_slice())),
                        msg = "P2P unexpected 24 bytes message",
                        address = tracing::field::display(&self.remote_address),
                    );
                    ConnectionState::Initial
                } else {
                    tracing::info!(
                        id = tracing::field::display(&message.event_id),
                        msg = "P2P receive connection messages in wrong order",        
                    );
                    ConnectionState::Unordered(message)
                }
            },
            // connection messages are in correct order
            ConnectionState::Initial => {
                self.send_message(message);
                ConnectionState::CorrectOrder
            },
            // both connection messages are already processed, it is a regular message
            ConnectionState::Completed | ConnectionState::CorrectOrder => {
                self.send_message(message);
                ConnectionState::Completed
            },
            // send stored message, and then current message, so they will be in correct order
            ConnectionState::Unordered(mut stored_message) => {
                let mut current_message = message;
                mem::swap(&mut current_message.counter, &mut stored_message.counter);
                self.send_message(stored_message);
                self.send_message(current_message);
                ConnectionState::Completed
            },
            ConnectionState::Invalid => ConnectionState::Invalid,
        };
        let _ = mem::replace(&mut self.state, state);
    }

    fn send_message(&mut self, message: p2p::Message) {
        match self.tx.send(Either::Left(message)) {
            Err(SendError(Either::Left(message))) => {
                tracing::error!(
                    id = tracing::field::display(&message.event_id),
                    incoming = message.incoming,
                    msg = "P2P Failed to forward message to the p2p parser",
                )
            },
            _ => (),
        }
    }
}
