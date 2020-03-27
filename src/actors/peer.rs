use failure::Error;
use riker::actors::*;
use std::{
    net::IpAddr,
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
    encoding::connection::ConnectionMessage,
};
use tezos_messages::p2p::binary_message::cache::CachedData;
use crate::storage::StoreMessage;

#[derive(Debug, Clone)]
/// Argument structure to create new peer
pub struct PeerArgs {
    pub addr: IpAddr,
    pub local_identity: Identity,
    pub db: MessageStore,
}

/// Actor representing communication over specific port, before proper communication is established.
pub struct Peer {
    db: MessageStore,
    addr: IpAddr,
    initialized: bool,
    incoming: bool,
    is_dead: bool,
    waiting: bool,
    conn_msgs: Vec<(ConnectionMessage, IpAddr)>,
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
            conn_msgs: Default::default(),
            peer_id: Default::default(),
            public_key: Default::default(),
            decrypter: None,
        }
    }

    fn process_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        if msg.is_push() {
            if self.initialized {
                self.process_encrypted_message(msg)
            } else {
                self.process_unencrypted_message(msg)
            }
        } else {
            self.process_control_message(msg)
        }
    }

    fn process_control_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        self.db.store_message(StoreMessage::new_tcp(msg))
    }

    fn process_unencrypted_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        // -> This *MUST* be one of connection messages exchanged between both nodes
        assert!(!self.initialized, "Connection trying to process encrypted messages as unencrypted");

        let chunk = BinaryChunk::try_from(msg.payload().to_vec())?;
        let conn_msg = ConnectionMessage::try_from(chunk)?;

        self.db.store_message(StoreMessage::new_conn(msg.source_addr(), msg.destination_addr(), &conn_msg))?;

        if let Some((_, addr)) = self.conn_msgs.get(0) {
            if addr == &msg.source_addr() {
                // Is duplicate
                log::info!("Got duplicit connection message message: {:?} @ {}", conn_msg, msg.source_addr());
            }
        }

        self.conn_msgs.push((conn_msg, msg.source_addr()));

        if self.conn_msgs.len() == 2 {
            self.initialized = true;
            self.upgrade()?;
            log::info!("Successfully upgraded peer {}",self.addr);
        }

        Ok(())
    }

    fn process_encrypted_message(&mut self, msg: &mut RawPacketMessage) -> Result<(), Error> {
        if let Some(ref mut decrypter) = self.decrypter {
            decrypter.recv_msg(msg)
        }
        Ok(())
    }

    fn upgrade(&mut self) -> Result<(), Error> {
        assert_eq!(self.conn_msgs.len(), 2, "trying to upgrade before all connection messages received");
        let ((first, _), (second, _)) = (&self.conn_msgs[0], &self.conn_msgs[1]);
        let first_pk = HashType::PublicKeyHash.bytes_to_string(&first.public_key());
        let is_incoming = first_pk != self.local_identity.public_key;
        let (received, sent) = if is_incoming {
            (second, first)
        } else {
            (first, second)
        };

        let sent_data = BinaryChunk::from_content(&sent.cache_reader().get().unwrap())?;
        let recv_data = BinaryChunk::from_content(&received.cache_reader().get().unwrap())?;

        let NoncePair { remote, .. } = generate_nonces(
            &sent_data.raw(),
            &recv_data.raw(),
            !is_incoming,
        );

        let remote_pk = HashType::PublicKeyHash.bytes_to_string(received.public_key());

        let precomputed_key = precompute(
            &hex::encode(received.public_key()),
            &self.local_identity.secret_key,
        )?;

        self.decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, remote, remote_pk.clone(), self.db.clone()));
        self.public_key = received.public_key().clone();
        self.peer_id = remote_pk;
        self.incoming = is_incoming;
        self.initialized = true;

        Ok(())
    }
}

impl Actor for Peer {
    type Msg = RawPacketMessage;

    fn recv(&mut self, _: &Context<RawPacketMessage>, mut msg: RawPacketMessage, _: Sender) {
        let _ = self.process_message(&mut msg);
    }
}
