use std::{net::SocketAddr, mem};
use tokio::{
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};
use futures::future::Either;

use super::{connection_parser::Parser, parser::{Command, Message}, report::ConnectionReport};
use crate::{
    storage_::{StoreCollector, p2p::Message as P2pMessage, indices::Initiator},
    system::utils::ReceiverStream,
};

pub struct Connection {
    state: ConnectionState,
    tx: mpsc::Sender<Either<Message, Command>>,
    handle: JoinHandle<ConnectionReport>,
    source_type: Initiator,
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
    Unordered(Message),
    Invalid,
}

impl Connection {
    pub fn spawn<S>(
        tx_report: mpsc::Sender<ConnectionReport>,
        parser: Parser<S>,
    ) -> Self
    where
        S: StoreCollector<Message = P2pMessage> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(0x10);
        let source_type = parser.source_type.clone();
        let remote_address = parser.remote_address.clone();
        let handle = tokio::spawn(parser.run(ReceiverStream::new(rx), tx_report));
        Connection {
            state: ConnectionState::Initial,
            tx,
            handle,
            source_type,
            remote_address,
        }
    }

    pub async fn process(&mut self, message: Message) {
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
                self.send_message(message).await;
                ConnectionState::CorrectOrder
            },
            // both connection messages are already processed, it is a regular message
            ConnectionState::Completed | ConnectionState::CorrectOrder => {
                self.send_message(message).await;
                ConnectionState::Completed
            },
            // send stored message, and then current message, so they will be in correct order
            ConnectionState::Unordered(mut stored_message) => {
                let mut current_message = message;
                mem::swap(&mut current_message.counter, &mut stored_message.counter);
                self.send_message(stored_message).await;
                self.send_message(current_message).await;
                ConnectionState::Completed
            },
            ConnectionState::Invalid => ConnectionState::Invalid,
        };
        let _ = mem::replace(&mut self.state, state);
    }

    async fn send(&mut self, item: Either<Message, Command>) {
        match self.tx.send(item).await {
            Err(SendError(Either::Left(message))) => {
                tracing::error!(
                    id = tracing::field::display(&message.event_id),
                    incoming = message.incoming,
                    msg = "P2P Failed to forward message to the p2p parser",
                )
            },
            Err(SendError(Either::Right(command))) => {
                tracing::error!(
                    command = tracing::field::debug(&command),
                    msg = "P2P Failed to forward command to the p2p parser",
                )
            },
            Ok(()) => (),
        }
    }

    async fn send_message(&mut self, message: Message) {
        self.send(Either::Left(message)).await
    }

    pub async fn send_command(&mut self, command: Command) {
        self.send(Either::Right(command)).await
    }

    pub async fn join(self) -> Option<ConnectionReport> {
        drop(self.tx);
        match self.handle.await {
            Ok(report) => Some(report),
            Err(error) => {
                tracing::error!(
                    error = tracing::field::display(&error),
                    msg = "P2P failed to join task which was processing the connection",
                );
                None
            },
        }
    }
}
