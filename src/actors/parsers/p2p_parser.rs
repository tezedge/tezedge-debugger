use failure::Error;
use riker::actors::*;
use std::net::{SocketAddr, IpAddr};
use crate::utility::tcp_packet::{Packet};
use crate::utility::identity::Identity;
use tezos_messages::p2p::encoding::prelude::ConnectionMessage;
use std::convert::TryFrom;
use crypto::hash::HashType;
use tezos_messages::p2p::binary_message::BinaryChunk;
use tezos_messages::p2p::binary_message::cache::CachedData;
use crypto::nonce::{NoncePair, generate_nonces};
use crypto::crypto_box::precompute;
use crate::utility::decrypter::P2PDecrypter;
use crate::utility::p2p_message::{P2PMessage};

#[derive(Debug, Clone)]
pub struct P2PParserArgs {
    pub addr: IpAddr,
    pub local_identity: Identity,
}

pub struct P2PParser {
    addr: IpAddr,
    initialized: bool,
    incoming: bool,
    conn_msgs: Vec<(ConnectionMessage, SocketAddr)>,
    handshake: u8,
    local_identity: Identity,
    peer_id: String,
    public_key: Vec<u8>,
    incoming_decrypter: Option<P2PDecrypter>,
    outgoing_decrypter: Option<P2PDecrypter>,
}

impl P2PParser {
    pub fn new(addr: IpAddr, local_identity: Identity) -> Self {
        Self {
            addr,
            local_identity,
            initialized: false,
            incoming: false,
            handshake: 0,
            conn_msgs: Vec::with_capacity(2),
            peer_id: Default::default(),
            public_key: Default::default(),
            incoming_decrypter: None,
            outgoing_decrypter: None,
        }
    }

    fn process_message(&mut self, msg: &mut Packet) -> Option<P2PMessage> {
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

    fn process_handshake_message(&mut self, _: &mut Packet) -> Option<P2PMessage> {
        self.handshake += 1;
        None
    }

    fn process_unencrypted_message(&mut self, msg: &mut Packet) -> Option<P2PMessage> {
        // -> This *MUST* be one of connection messages exchanged between both nodes
        assert!(!self.initialized, "Connection trying to process encrypted messages as unencrypted");

        let chunk = BinaryChunk::try_from(msg.payload().to_vec()).ok()?;
        let conn_msg = ConnectionMessage::try_from(chunk).ok()?;

        if let Some((_, addr)) = self.conn_msgs.get(0) {
            if addr == &msg.source_addr() {
                // Is duplicate
                log::info!("Got duplicate connection message message: {:?} @ {}", conn_msg, msg.source_addr());
            }
        }

        self.conn_msgs.push((conn_msg.clone(), msg.source_addr()));

        if self.conn_msgs.len() == 2 {
            self.upgrade().ok()?;
            log::info!("Successfully upgraded peer {}", self.peer_id);
        }

        let (remote, incoming) = if msg.destination_address().ip() == self.addr {
            (msg.source_addr(), true)
        } else {
            (msg.destination_address(), false)
        };

        Some(P2PMessage::new(remote, incoming, vec![conn_msg]))
    }

    /// Process encrypted messages, after securing connection between nodes.
    fn process_encrypted_message(&mut self, msg: &mut Packet) -> Option<P2PMessage> {
        let (decrypter, remote, incoming) = if msg.destination_address().ip() == self.addr {
            (&mut self.incoming_decrypter, msg.source_addr(), true)
        } else {
            (&mut self.outgoing_decrypter, msg.destination_address(), false)
        };

        if let Some(ref mut decrypter) = decrypter {
            let msgs = decrypter.recv_msg(msg);
            if let Some(msgs) = msgs {
                Some(P2PMessage::new(remote, incoming, msgs))
            } else {
                None
            }
        } else {
            None
        }
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

        self.incoming_decrypter = Some(P2PDecrypter::new(precomputed_key.clone(), remote));
        self.outgoing_decrypter = Some(P2PDecrypter::new(precomputed_key.clone(), local));
        self.public_key = received.public_key.clone();
        self.peer_id = remote_pk;
        self.incoming = is_incoming;
        self.initialized = true;

        Ok(())
    }
}

impl Actor for P2PParser {
    type Msg = Packet;

    fn recv(&mut self, ctx: &Context<Self::Msg>, mut msg: Self::Msg, _: Sender) {
        let msg = self.process_message(&mut msg);
        if let Some(msg) = msg {
            match ctx.select("/user/processors/*") {
                Ok(actor_ref) => actor_ref.try_tell(msg, ctx.myself()),
                Err(err) => log::error!("Failed to propagate parsed p2p message: {}", err),
            }
        }
    }
}

impl ActorFactoryArgs<P2PParserArgs> for P2PParser {
    fn create_args(args: P2PParserArgs) -> Self {
        Self::new(args.addr, args.local_identity)
    }
}