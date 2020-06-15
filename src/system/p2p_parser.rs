use tokio::sync::mpsc::{UnboundedSender, unbounded_channel, UnboundedReceiver};
use tracing::{trace, info, error};
use failure::Error;
use crypto::{
    crypto_box::precompute,
    nonce::{NoncePair, generate_nonces},
};
use std::{
    convert::TryFrom,
    net::{SocketAddr, IpAddr},
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, cache::CachedData},
    encoding::prelude::*,
};
use crate::{
    utility::prelude::*,
    messages::prelude::*,
};

struct Parser {
    pub initializer: SocketAddr,
    receiver: UnboundedReceiver<Packet>,
    processor_sender: Option<UnboundedSender<P2pMessage>>,
    encryption: ParserEncryption,
    state: ParserState,
}

impl Parser {
    fn new(initializer: SocketAddr, receiver: UnboundedReceiver<Packet>) -> Self {
        Self {
            initializer,
            receiver,
            encryption: ParserEncryption::new(initializer),
            processor_sender: None,
            state: ParserState::Unencrypted,
        }
    }

    async fn parse_next(&mut self) -> bool {
        match self.receiver.recv().await {
            Some(packet) => {
                trace!(process_length = packet.ip_buffer().len(), "processing packet");
                self.parse(packet).await
            }
            None => {
                error!("p2p parser channel closed abruptly");
                false
            }
        }
    }

    async fn parse(&mut self, packet: Packet) -> bool {
        if packet.is_closing() {
            false
        } else {
            if packet.has_payload() {
                let p2p_msg = match self.state {
                    ParserState::Unencrypted => self.parse_unencrypted(packet).await,
                    ParserState::Encrypted => self.parse_encrypted(packet).await,
                    _ => { return true; }
                };
                if let Some(p2p_msg) = p2p_msg {
                    if self.processor_sender.as_ref().unwrap().send(p2p_msg).is_err() {
                        error!("processor channel closed abruptly");
                        return false;
                    }
                }
            }
            true
        }
    }

    async fn parse_unencrypted(&mut self, packet: Packet) -> Option<P2pMessage> {
        match self.encryption.process_unencrypted(packet) {
            Ok(result) => {
                if self.encryption.is_initialized() {
                    self.state = ParserState::Encrypted;
                }
                result
            }
            Err(err) => {
                info!(addr = display(self.initializer), error = display(err), "is not valid tezos p2p connection");
                self.state = ParserState::Irrelevant;
                None
            }
        }
    }

    async fn parse_encrypted(&mut self, packet: Packet) -> Option<P2pMessage> {
        if !self.encryption.is_initialized() {
            self.parse_unencrypted(packet).await
        } else {
            match self.encryption.process_encrypted(packet) {
                Ok(result) => result,
                Err(err) => {
                    trace!(addr = display(self.initializer), error = display(err), "received invalid message");
                    self.state = ParserState::Irrelevant;
                    None
                }
            }
        }
    }
}

pub fn spawn_p2p_parser(initializer: SocketAddr) -> UnboundedSender<Packet> {
    let (sender, receiver) = unbounded_channel::<Packet>();
    tokio::spawn(async move {
        let mut parser = Parser::new(initializer, receiver);
        while parser.parse_next().await {
            trace!(addr = display(initializer), "parsed new message");
        }
        info!(addr = display(initializer), "parser received closing packet");
    });
    sender
}

enum ParserState {
    // Nodes did not exchanged Connection messages yet
    Unencrypted,
    // Nodes did exchanged connection messages
    Encrypted,
    // Is not connection containing tezos p2p communication, ignore it
    Irrelevant,
}

pub struct ParserEncryption {
    initializer: SocketAddr,
    local_address: IpAddr,
    local_identity: Identity,
    first_connection_message: Option<(ConnectionMessage, SocketAddr)>,
    second_connection_message: Option<(ConnectionMessage, SocketAddr)>,
    incoming_decrypter: Option<P2pDecrypter>,
    outgoing_decrypter: Option<P2pDecrypter>,
}

impl ParserEncryption {
    pub fn new(initializer: SocketAddr) -> Self {
        Self {
            initializer,
            local_address: "0.0.0.0".parse().unwrap(),
            local_identity: Default::default(),
            first_connection_message: None,
            second_connection_message: None,
            incoming_decrypter: None,
            outgoing_decrypter: None,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.incoming_decrypter.is_some() && self.outgoing_decrypter.is_some()
    }

    pub fn extract_remote(&self, packet: &Packet) -> (SocketAddr, bool) {
        let incoming = self.local_address == packet.destination_address().ip();
        (if incoming { packet.source_addr() } else { packet.destination_address() }, incoming)
    }

    pub fn process_unencrypted(&mut self, packet: Packet) -> Result<Option<P2pMessage>, Error> {
        if self.is_initialized() {
            Ok(None)
        } else {
            let chunk = BinaryChunk::try_from(packet.payload().to_vec())?;
            let conn_msg = ConnectionMessage::try_from(chunk)?;
            trace!(message_length = packet.payload().len(), "processed unencrypted message");
            let mut upgrade = false;
            let (remote, incoming) = self.extract_remote(&packet);

            let place = if let Some((_, addr)) = self.first_connection_message {
                if addr == packet.source_addr() {
                    info!(addr = display(addr), "received duplicate connection message");
                    return Ok(Some(P2pMessage::new(remote, incoming, vec![conn_msg])));
                } else {
                    upgrade = true;
                    &mut self.second_connection_message
                }
            } else {
                &mut self.first_connection_message
            };
            *place = Some((conn_msg.clone(), packet.source_addr()));

            if upgrade {
                self.upgrade()?;
            }

            Ok(Some(P2pMessage::new(remote, incoming, vec![conn_msg])))
        }
    }

    pub fn process_encrypted(&mut self, packet: Packet) -> Result<Option<P2pMessage>, Error> {
        let (remote, incoming) = self.extract_remote(&packet);
        let decrypter = if packet.destination_address() == self.initializer {
            &mut self.incoming_decrypter
        } else {
            &mut self.outgoing_decrypter
        };

        Ok(decrypter.as_mut()
            .map(|decrypter| decrypter.recv_msg(&packet)).flatten()
            .map(|msgs| {
                trace!(message_len = packet.payload().len(), "processed encrypted message");
                P2pMessage::new(remote, incoming, msgs)
            }))
    }

    pub fn upgrade(&mut self) -> Result<(), Error> {
        if let (Some((sent, _)), Some((received, _))) = (&self.first_connection_message, &self.second_connection_message) {
            let sent_data = BinaryChunk::from_content(&sent.cache_reader().get().unwrap())?;
            let recv_data = BinaryChunk::from_content(&received.cache_reader().get().unwrap())?;

            let NoncePair { remote, local } = generate_nonces(
                &sent_data.raw(),
                &recv_data.raw(),
                true,
            );

            let precomputed_key = precompute(
                &hex::encode(&received.public_key),
                &self.local_identity.secret_key,
            )?;

            self.incoming_decrypter = Some(P2pDecrypter::new(precomputed_key.clone(), remote));
            self.outgoing_decrypter = Some(P2pDecrypter::new(precomputed_key.clone(), local));

            info!(initializer = display(self.initializer), "connection upgraded to encrypted");
            Ok(())
        } else {
            // TODO: return error
            unreachable!()
        }
    }
}