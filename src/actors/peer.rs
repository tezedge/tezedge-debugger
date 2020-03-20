use failure::Error;
use riker::actors::*;
use std::{
    net::Ipv4Addr,
    convert::{TryFrom, TryInto},
};
use crypto::{
    hash::HashType,
    crypto_box::precompute,
    nonce::{NoncePair, generate_nonces},
};
use crate::{
    network::prelude::*,
    configuration::Identity,
    storage::MessageStore,
};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
    encoding::connection::ConnectionMessage,
};
use crate::actors::packet_orchestrator::{Packet, OrchestratorMessage};

#[derive(Clone, Debug, PartialEq)]
/// Message representing a network message for a peer.
pub enum PeerMessage {
    Inner(Packet),
    Outer(Packet),
}

#[derive(Debug, Clone)]
/// Argument structure to create new peer
pub struct PeerArgs {
    pub addr: Ipv4Addr,
    pub local_identity: Identity,
    pub db: MessageStore,
}

/// Actor representing communication over specific port, before proper communication is established.
pub struct Peer {
    db: MessageStore,
    addr: Ipv4Addr,
    initialized: bool,
    incoming: bool,
    is_dead: bool,
    waiting: bool,
    buf: Vec<Packet>,
    local_identity: Identity,
    peer_id: String,
    public_key: Vec<u8>,
    decrypter: Option<EncryptedMessageDecoder>,
}

impl Peer {
    pub fn new(args: PeerArgs) -> Self {
        Self {
            db: args.db,
            addr: args.addr,
            local_identity: args.local_identity,
            initialized: false,
            incoming: false,
            is_dead: false,
            waiting: false,
            buf: Default::default(),
            peer_id: Default::default(),
            public_key: Default::default(),
            decrypter: None,
        }
    }

    fn try_upgrade(&mut self) -> Result<bool, Error> {
        if let Some((first, second)) = self.handshake() {
            let is_incoming = first.is_incoming();
            let (received, sent) = if is_incoming {
                (first, second)
            } else {
                (second, first)
            };
            let (received, sent): (BinaryChunk, BinaryChunk) = (
                received.raw_msg().to_vec().try_into()?,
                sent.raw_msg().to_vec().try_into()?,
            );
            let NoncePair { remote: remote_nonce, .. } = generate_nonces(
                sent.raw(),
                received.raw(),
                is_incoming,
            );

            let peer_conn_msg: ConnectionMessage = ConnectionMessage::try_from(received)?;
            let public_key = peer_conn_msg.public_key();
            let peer_id = HashType::PublicKeyHash.bytes_to_string(&public_key);
            let precomputed_key = precompute(&hex::encode(&public_key), &self.local_identity.secret_key)?;

            self.decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, remote_nonce, peer_id.clone(), self.db.clone()));
            self.public_key = public_key.clone();
            self.peer_id = peer_id;
            self.incoming = is_incoming;
            self.initialized = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn handshake(&self) -> Option<(&Packet, &Packet)> {
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

    fn check_packet(&self, packet: &Packet) -> bool {
        !packet.is_empty() && self.is_dead
    }

    fn remote_process_packet(&mut self, packet: Packet) -> Packet {
        if let Some(ref mut decrypter) = self.decrypter {
            decrypter.recv_msg(&packet)
        }
        packet
    }

    fn primer_process_packet(&mut self, mut packet: Packet) -> Packet {
        if !self.check_packet(&packet) {
            return packet;
        }

        self.buf.push(packet.clone());
        if self.buf.len() == 2 && packet.is_incoming() {
            // This is remote connection message, should be replaced local one
            let mut fake_packet = self.buf.get(0).unwrap().clone();
            fake_packet.packet_mut().set_source(packet.packet().get_source());
            fake_packet.packet_mut().set_destination(packet.packet().get_destination());
            packet = fake_packet
        }
        if !self.initialized && self.buf.len() >= 2 {
            match self.try_upgrade() {
                Ok(true) => {
                    log::info!("Successfully upgraded port {} to {} peer {} (with {} messages)", self.addr, if self.incoming {
                        "incoming"
                    } else {
                        "outgoing"
                    }, self.peer_id, self.buf.len());
                    self.buf.clear();
                    self.buf.shrink_to_fit();
                }
                Err(e) => {
                    self.is_dead = true;
                    let (first, second) = (self.buf.get(0).unwrap(), self.buf.get(1).unwrap());
                    let is_incoming = first.is_incoming();
                    let (inc, out) = if is_incoming {
                        (first, second)
                    } else {
                        (second, first)
                    };
                    log::error!("Failed to upgrade client on port {}. Handshake messages:\nTezedge: \
                        \n\t{:?}\nOCaml:\n\t{:?}\nError: {}", self.addr, out, inc, e);
                }
                _ => {
                    if !self.waiting {
                        self.waiting = true;
                        log::info!("Peer peer-{} stuck at waiting for handshake (buffer: {})", self.addr.to_string().replace(".", "_"), self.buf.len())
                    }
                }
            }
        }
        packet
    }
}

impl Actor for Peer {
    type Msg = PeerMessage;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        match msg {
            PeerMessage::Inner(packet) => {
                assert!(packet.is_outgoing(), "peer can only receive inner-outgoing or outer-incoming communication");
                let packet = if self.initialized {
                    self.remote_process_packet(packet)
                } else {
                    self.primer_process_packet(packet)
                };

                if let Some(actor) = sender {
                    let sender: BasicActorRef = ctx.myself().into();
                    let _ = actor.try_tell(OrchestratorMessage::Outer(packet), sender);
                }
            }
            PeerMessage::Outer(packet) => {
                assert!(packet.is_incoming(), "peer can only receive inner-outgoing or outer-incoming communication");
                // TODO: Return correctly processed packet out of process function to be re-transmitted
                let packet = if self.initialized {
                    self.remote_process_packet(packet)
                } else {
                    self.primer_process_packet(packet)
                };

                if let Some(actor) = sender {
                    let sender: BasicActorRef = ctx.myself().into();
                    let _ = actor.try_tell(OrchestratorMessage::Inner(packet), sender);
                }
            }
        }
    }
}
