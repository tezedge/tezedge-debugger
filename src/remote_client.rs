use crate::network_message::NetworkMessage;
use crossbeam::{
    Sender, Receiver,
    channel::unbounded,
};
use std::{
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    convert::TryFrom,
};
use log;
use failure::Error;
use crypto::nonce::{generate_nonces, NoncePair};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::binary_message::BinaryChunk;
use crypto::hash::HashType;
use crypto::crypto_box::precompute;
use itertools::Itertools;
use crate::msg_decoder::EncryptedMessageDecoder;

enum Control<T> {
    Msg(T),
    Die,
}

pub struct RemoteClient {
    id: u16,
    peer_id: String,
    chan: Sender<Control<NetworkMessage>>,
    handle: Option<JoinHandle<()>>,
    decrypter: Option<EncryptedMessageDecoder>,
    msg_buf: Vec<NetworkMessage>,
}

impl RemoteClient {
    pub fn spawn(id: u16) -> Arc<RwLock<Self>> {
        let (chan, tx) = unbounded::<Control<NetworkMessage>>();
        let ret = Arc::new(RwLock::new(Self {
            id,
            chan,
            handle: None,
            peer_id: String::new(),
            decrypter: None,
            msg_buf: Vec::default(),
        }));
        let client = ret.clone();
        let handle = thread::spawn(move || Self::handle_controls(client, tx));
        let mut lock = ret.write().expect("Lock poisoning");
        lock.handle = Some(handle);
        drop(lock);
        ret
    }

    pub fn send_message(&mut self, message: NetworkMessage) {
        self.chan.send(Control::Msg(message)).expect("channel failure")
    }

    pub fn stop(&mut self) {
        self.chan.send(Control::Die).expect("channel failure")
    }

    pub fn has_decrypter(&self) -> bool {
        self.decrypter.is_some()
    }

    fn initialize_reader(&mut self) -> Result<(), Error> {
        use crate::IDENTITY;

        for perm in self.msg_buf.iter().permutations(2) {
            let incoming_nonce = perm.get(0).expect("expected 2 element permutations");
            let outgoing_nonce = perm.get(1).expect("expected 2 element permutations");
            let NoncePair { remote: nonce_remote, .. } = generate_nonces(outgoing_nonce.raw_msg(), incoming_nonce.raw_msg(), false);
            let chunk = BinaryChunk::from_content(incoming_nonce.raw_msg())?;
            let peer_connection_message: ConnectionMessage = ConnectionMessage::try_from(chunk)?;
            let peer_public_key = peer_connection_message.public_key();
            self.peer_id = HashType::PublicKeyHash.bytes_to_string(&peer_public_key);

            let precomputed_key = match precompute(&hex::encode(peer_public_key), &IDENTITY.secret_key) {
                Ok(key) => key,
                Err(_) => panic!("Failed at calculating precomputed key"),
            };

            self.decrypter = Some(EncryptedMessageDecoder::new(precomputed_key, nonce_remote, self.peer_id.clone()));
        }
        log::info!("Successfully created message decoder");
        Ok(())
    }

    fn handle_controls(client: Arc<RwLock<Self>>, receiver: Receiver<Control<NetworkMessage>>) {
        for msg in receiver.iter() {
            match msg {
                Control::Msg(msg) => {
                    Self::process_message(client.clone(), msg);
                }
                Control::Die => {
                    break;
                }
            }
        }
    }

    fn process_message(client: Arc<RwLock<Self>>, msg: NetworkMessage) {
        let self_ref = client.read().expect("Lock poisoning");
        let has_nonces = self_ref.msg_buf.len() == 2;
        drop(self_ref);
        if !has_nonces {
            Self::process_nonce(&client, msg);
        } else {
            let mut self_ref = client.write().expect("Lock poisoning");
            if let Some(ref mut decrypt) = self_ref.decrypter {
                decrypt.recv_msg(msg)
            }
        }
    }

    fn process_nonce(client: &Arc<RwLock<Self>>, msg: NetworkMessage) {
        let mut self_ref = client.write().expect("Lock poisoning");

        if !msg.is_empty() {
            self_ref.msg_buf.push(msg);
        }

        if !self_ref.has_decrypter() {
            if let Err(e) = self_ref.initialize_reader() {
                log::error!("Failed to create decrypter: {}", e);
            }
        }
    }
}