// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use riker::actors::*;
use std::{
    net::SocketAddr,
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
    actors::peer_message::*,
};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
};
use tezos_messages::p2p::binary_message::cache::CachedData;
use crate::{
    network::connection_message::ConnectionMessage,
    storage::StoreMessage,
};

#[derive(Clone)]
/// Argument structure to create new P2P message processor
pub struct PeerArgs {
    pub addr: SocketAddr,
    pub local_identity: Identity,
    pub db: MessageStore,
}

/// Actor representing/processing communication with specific remote node identified by Socket Address.
pub struct PeerProcessor {
    db: MessageStore,
    addr: SocketAddr,
    initialized: bool,
    incoming: bool,
    is_dead: bool,
    waiting: bool,
    conn_msgs: Vec<(ConnectionMessage, SocketAddr)>,
    handshake: u8,
    local_identity: Identity,
    peer_id: String,
    public_key: Vec<u8>,
    incoming_decrypter: Option<EncryptedMessageDecoder>,
    outgoing_decrypter: Option<EncryptedMessageDecoder>,
}

impl PeerProcessor {
    /// Create new Processor from given args
    pub fn new(args: PeerArgs) -> Self {
        Self {
            db: args.db,
            addr: args.addr,
            local_identity: args.local_identity,
            handshake: 0,
            initialized: false,
            incoming: false,
            is_dead: false,
            waiting: false,
            conn_msgs: Default::default(),
            peer_id: Default::default(),
            public_key: Default::default(),
            incoming_decrypter: None,
            outgoing_decrypter: None,
        }
    }

    /// Process given message (check its content, and if needed decypher/deserialize/store it)
    fn process_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        if self.handshake == 3 {
            if self.initialized {
                self.process_encrypted_message(msg)
            } else {
                self.process_unencrypted_message(msg)
            }
        } else {
            self.process_handshake_message(msg)
        }
    }


    /// Process TCP handshake message
    fn process_handshake_message(&mut self, _msg: &mut RawPacketMessage) -> Result<(), Error> {
        self.handshake += 1;
        // Disable Raw TCP packet storing for now
        // self.db.store_p2p_message(&StoreMessage::new_tcp(&msg), msg.remote_addr())
        Ok(())
    }

    /// Process non-TCP handshake message included in tezos bootstrap (handshake) process
    fn process_unencrypted_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        // -> This *MUST* be one of connection messages exchanged between both nodes
        assert!(!self.initialized, "Connection trying to process encrypted messages as unencrypted");

        let chunk = BinaryChunk::try_from(msg.payload().to_vec())?;
        let conn_msg = ConnectionMessage::try_from(chunk)?;

        self.db.p2p().store_message(&StoreMessage::new_connection(msg.remote_addr(), msg.is_incoming(), &conn_msg))?;

        if let Some((_, addr)) = self.conn_msgs.get(0) {
            if addr == &msg.source_addr() {
                // Is duplicate
                log::info!("Got duplicit connection message message: {:?} @ {}", conn_msg, msg.source_addr());
            }
        }

        self.conn_msgs.push((conn_msg, msg.source_addr()));

        if self.conn_msgs.len() == 2 {
            self.upgrade()?;
            log::info!("Successfully upgraded peer {}",self.addr);
        }

        Ok(())
    }

    /// Process encrypted messages, after securing connection between nodes.
    fn process_encrypted_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        let decrypter = if msg.is_incoming() {
            &mut self.incoming_decrypter
        } else {
            &mut self.outgoing_decrypter
        };
        if let Some(ref mut decrypter) = decrypter {
            decrypter.recv_msg(msg)
        }
        Ok(())
    }

    /// Upgrade processor to encrypted state
    fn upgrade(&mut self) -> Result<(), Error> {
        assert_eq!(self.conn_msgs.len(), 2, "trying to upgrade before all connection messages received");
        let ((first, _), (second, _)) = (&self.conn_msgs[0], &self.conn_msgs[1]);
        let first_pk = HashType::CryptoboxPublicKeyHash.bytes_to_string(&first.public_key);
        let is_incoming = first_pk != self.local_identity.public_key;
        let (received, sent) = if is_incoming {
            (second, first)
        } else {
            (first, second)
        };

        let sent_data = BinaryChunk::from_content(&sent.cache_reader().get().unwrap())?;
        let recv_data = BinaryChunk::from_content(&received.cache_reader().get().unwrap())?;

        let NoncePair { remote, local } = generate_nonces(
            &sent_data.raw(),
            &recv_data.raw(),
            !is_incoming,
        );

        let remote_pk = HashType::CryptoboxPublicKeyHash.bytes_to_string(&received.public_key);

        let precomputed_key = precompute(
            &hex::encode(&received.public_key),
            &self.local_identity.secret_key,
        )?;

        self.incoming_decrypter = Some(EncryptedMessageDecoder::new(precomputed_key.clone(), remote, remote_pk.clone(), self.db.clone()));
        self.outgoing_decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, local, remote_pk.clone(), self.db.clone()));
        self.public_key = received.public_key.clone();
        self.peer_id = remote_pk;
        self.incoming = is_incoming;
        self.initialized = true;

        Ok(())
    }
}

impl Actor for PeerProcessor {
    type Msg = RawPacketMessage;

    fn recv(&mut self, ctx: &Context<RawPacketMessage>, mut msg: RawPacketMessage, sender: Sender) {
        let _ = self.process_message(&mut msg);
        if let Some(sender) = sender {
            msg.flip_side();
            if let Err(_) = sender.try_tell(SenderMessage::Relay(msg), ctx.myself()) {
                log::error!("unable to reach packet orchestrator with processed packet")
            }
        }
    }
}
