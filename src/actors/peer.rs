use failure::Error;
use riker::actors::*;
use std::{
    net::Ipv4Addr,
    convert::TryFrom,
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
use pnet::packet::Packet as _;
use packet::{
    Packet as _,
    ip::v4::Packet as Ipv4Packet,
    tcp::Packet as TcpPacket,
};
use tezos_messages::p2p::binary_message::cache::CachedData;
use crate::storage::StoreMessage;

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
    buf: Vec<ConnectionMessage>,
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
        if self.buf.len() == 2 {
            let precomputed_key;
            let remote_nonce;
            let peer_id;
            let is_incoming;
            let public_key;
            {
                let (first, second) = self.handshake().unwrap();
                let first_pk = HashType::PublicKeyHash.bytes_to_string(&first.public_key());
                is_incoming = first_pk != self.local_identity.public_key;
                let (received, sent) = if is_incoming {
                    (first, second)
                } else {
                    (second, first)
                };

                let NoncePair { remote, .. } = generate_nonces(
                    &sent.cache_reader().get().unwrap(),
                    &received.cache_reader().get().unwrap(),
                    is_incoming,
                );
                remote_nonce = remote;

                public_key = received.public_key().clone();
                peer_id = HashType::PublicKeyHash.bytes_to_string(&public_key);
                precomputed_key = precompute(&hex::encode(&public_key), &self.local_identity.secret_key)?;
            }

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

    fn handshake(&self) -> Option<(&ConnectionMessage, &ConnectionMessage)> {
        if self.buf.len() >= 2 {
            Some((self.buf.get(0).unwrap(), self.buf.get(1).unwrap()))
        } else {
            None
        }
    }

    fn check_packet(&self, packet: &Packet) -> bool {
        use packet::tcp::flag::PSH;
        let raw = &packet.packet().packet();
        let ipp = Ipv4Packet::new(raw).unwrap();
        let tpp = TcpPacket::new(ipp.payload()).unwrap();
        !packet.is_empty() && !self.is_dead
            && tpp.flags().intersects(PSH)
    }

    fn remote_process_packet(&mut self, packet: Packet) -> Packet {
        if let Some(ref mut decrypter) = self.decrypter {
            decrypter.recv_msg(&packet)
        }
        packet
    }

    fn primer_process_packet(&mut self, packet: Packet) -> Packet {
        if !self.check_packet(&packet) {
            let _ = self.db.store_message(packet.clone().into());
            return packet;
        }

        if self.buf.len() == 0 || self.buf.len() == 1 {
            let p = packet.packet().packet();
            let ipp = Ipv4Packet::new(p).unwrap();
            let tpp = TcpPacket::new(ipp.payload()).unwrap();
            if let Ok(chunk) = BinaryChunk::try_from(tpp.payload().to_vec()) {
                if let Ok(msg) = ConnectionMessage::try_from(chunk) {
                    let _ = self.db.store_message(StoreMessage::new_conn(
                        packet.packet().get_source(),
                        packet.packet().get_destination(),
                        &msg,
                    ));
                    self.buf.push(msg);
                }
            }
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
                    log::info!("Failed to upgrade peer {}: {}", self.addr, e);
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
