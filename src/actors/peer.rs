use std::convert::TryFrom;
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
    buf: Vec<NetworkMessage>,
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
            buf: Default::default(),
            peer_id: Default::default(),
            public_key: Default::default(),
            decrypter: None,
        }
    }

    fn remote_process_packet(&mut self, _msg: NetworkMessage) {}

    fn primer_process_packet(&mut self, msg: NetworkMessage) {
        self.buf.push(msg);
        match self.try_upgrade() {
            Ok(true) => log::warn!("Successfully upgraded port {} to {} peer {} ", self.port, if self.incoming {
                "incoming"
            } else {
                "outgoing"
            }, self.peer_id),
            _ => {
                return;
            }
        }
    }

    fn try_upgrade(&mut self) -> Result<bool, Error> {
        if self.buf.len() > 2 {
            for i in 0..self.buf.len() - 1 {
                for j in i + 1..self.buf.len() {
                    let (first, second) = (self.buf.get(i).unwrap(), self.buf.get(j).unwrap());
                    let is_incoming = first.is_incoming();
                    let (incoming, outgoing) = if first.is_incoming() {
                        (first, second)
                    } else {
                        (second, first)
                    };
                    let NoncePair { remote: remote_nonce, .. } = generate_nonces(outgoing.raw_msg(), incoming.raw_msg(), is_incoming);
                    let chunk = BinaryChunk::from_content(incoming.raw_msg())?;
                    let peer_conn_msg: ConnectionMessage = ConnectionMessage::try_from(chunk)?;
                    let public_key = peer_conn_msg.public_key();
                    let peer_id = HashType::PublicKeyHash.bytes_to_string(&public_key);
                    let precomputed_key = precompute(&hex::encode(&public_key), &self.local_identity.secret_key)?;

                    self.decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, remote_nonce, peer_id.clone()));
                    self.public_key = public_key.clone();
                    self.peer_id = peer_id;
                    self.incoming = is_incoming;
                    self.initialized = true;

                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl Actor for Peer {
    type Msg = Message;

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Sender) {
        if !msg.0.is_empty() {
            if self.initialized {
                self.remote_process_packet(msg.0);
            } else {
                self.primer_process_packet(msg.0);
            }
        }
    }
}
