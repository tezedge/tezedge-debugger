use std::convert::{TryFrom, TryInto};
use failure::Error;
use riker::actors::*;
use crypto::{
    hash::HashType,
    crypto_box::precompute,
    nonce::{NoncePair, generate_nonces},
};
use crate::{
    network::prelude::*,
    configuration::Identity,
};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::connection::ConnectionMessage,
};

#[derive(Clone, Debug, PartialEq)]
/// Message representing a network message for a peer.
pub struct Message(NetworkMessage);

impl Message {
    pub fn new(msg: NetworkMessage) -> Self {
        Self(msg)
    }
}

#[derive(Debug, Clone)]
/// Argument structure to create new peer
pub struct PeerArgs {
    pub port: u16,
    pub local_identity: Identity,
}

/// Actor representing communication over specific port, before proper communication is established.
pub struct Peer {
    port: u16,
    initialized: bool,
    incoming: bool,
    dead: bool,
    waiting: bool,
    buf: Vec<NetworkMessage>,
    inc_buf: Vec<u8>,
    out_buf: Vec<u8>,
    local_identity: Identity,
    peer_id: String,
    public_key: Vec<u8>,
    decrypter: Option<EncryptedMessageDecoder>,
}

impl Peer {
    pub fn new(args: PeerArgs) -> Self {
        Self {
            port: args.port,
            local_identity: args.local_identity,
            initialized: false,
            incoming: false,
            dead: false,
            waiting: false,
            buf: Default::default(),
            inc_buf: Default::default(),
            out_buf: Default::default(),
            peer_id: Default::default(),
            public_key: Default::default(),
            decrypter: None,
        }
    }

    fn remote_process_packet(&mut self, msg: NetworkMessage) {
        if let Some(ref mut decrypter) = self.decrypter {
            decrypter.recv_msg(msg)
        }
    }

    fn primer_process_packet(&mut self, msg: NetworkMessage) {
        self.buf.push(msg);
        if !self.initialized && self.buf.len() >= 2 {
            match self.try_upgrade() {
                Ok(true) => log::info!("Successfully upgraded port {} to {} peer {} (with {} messages)", self.port, if self.incoming {
                        "incoming"
                    } else {
                        "outgoing"
                    }, self.peer_id, self.buf.len()
                ),
                Err(e) => {
                    self.dead = true;
                    let (first, second) = (self.buf.get(0).unwrap(), self.buf.get(1).unwrap());
                    let is_incoming = first.is_incoming();
                    let (inc, out) = if is_incoming {
                        (first, second)
                    } else {
                        (second, first)
                    };
                    log::error!("Failed to upgrade client on port {}. Handshake messages:\nTezedge: \
                        \n\t{:?}\nOCaml:\n\t{:?}\nError: {}", self.port, out, inc, e);
                }
                _ => {
                    if !self.waiting {
                        self.waiting = true;
                        log::info!("Peer {} stuck at waiting for handshake (buffer: {})", self.port, self.buf.len())
                    }
                }
            }
        }
    }

    fn try_upgrade(&mut self) -> Result<bool, Error> {
        if let Some((first, second)) = self.handshake() {
            let is_incoming = first.is_incoming();
            let (incoming, outgoing) = if is_incoming {
                (first, second)
            } else {
                (second, first)
            };
            let (incoming, outgoing): (BinaryChunk, BinaryChunk) = (
                incoming.raw_msg().to_vec().try_into()?,
                outgoing.raw_msg().to_vec().try_into()?,
            );
            let NoncePair { remote: remote_nonce, .. } = generate_nonces(
                outgoing.raw(),
                incoming.raw(),
                is_incoming,
            );

            let peer_conn_msg: ConnectionMessage = ConnectionMessage::try_from(incoming)?;
            let public_key = peer_conn_msg.public_key();
            let peer_id = HashType::PublicKeyHash.bytes_to_string(&public_key);
            let precomputed_key = precompute(&hex::encode(&public_key), &self.local_identity.secret_key)?;

            self.decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, remote_nonce, peer_id.clone()));
            self.public_key = public_key.clone();
            self.peer_id = peer_id;
            self.incoming = is_incoming;
            self.initialized = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn handshake(&self) -> Option<(&NetworkMessage, &NetworkMessage)> {
        if self.buf.len() >= 2 {
            let first = self.buf.get(0).unwrap();
            if let Some(second) = self.buf.iter().find(|x| x.is_incoming() != first.is_incoming()) {
                Some((first, second))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Actor for Peer {
    type Msg = Message;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        if !msg.0.is_empty() && !self.dead {
            if self.initialized {
                self.remote_process_packet(msg.0);
            } else {
                self.primer_process_packet(msg.0);
            }
        }
    }
}
