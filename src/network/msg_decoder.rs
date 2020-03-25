use crypto::{
    crypto_box::{PrecomputedKey, decrypt},
    nonce::Nonce,
};
use tezos_encoding::{
    binary_reader::BinaryReaderError
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, BinaryMessage},
    encoding::peer::PeerMessageResponse,
};
use std::convert::TryFrom;
use bytes::Buf;
use crate::actors::packet_orchestrator::Packet;
use crate::storage::MessageStore;

pub struct EncryptedMessageDecoder {
    db: MessageStore,
    precomputed_key: PrecomputedKey,
    remote_nonce: Nonce,
    peer_id: String,
    processing: bool,
    enc_buf: Vec<u8>,
    dec_buf: Vec<u8>,
    input_remaining: usize,
}

impl EncryptedMessageDecoder {
    pub fn new(precomputed_key: PrecomputedKey, remote_nonce: Nonce, peer_id: String, db: MessageStore) -> Self {
        Self {
            db,
            precomputed_key,
            remote_nonce,
            peer_id,
            processing: false,
            enc_buf: Default::default(),
            dec_buf: Default::default(),
            input_remaining: 0,
        }
    }

    pub fn recv_msg(&mut self, enc: &Packet) {
        if enc.is_incoming() && !enc.is_empty() {
            if self.enc_buf.is_empty() {
                self.enc_buf.extend_from_slice(&enc.raw_msg());
            } else {
                self.enc_buf.extend_from_slice(&enc.raw_msg()[2..]);
            }
            self.try_decrypt();
        }
    }

    fn try_decrypt(&mut self) {
        let len = (&self.enc_buf[0..2]).get_u16() as usize;
        if self.enc_buf[2..].len() >= len {
            let chunk = match BinaryChunk::try_from(self.enc_buf[0..len + 2].to_vec()) {
                Ok(chunk) => chunk,
                Err(e) => {
                    log::error!("Failed to load binary chunk: {}", e);
                    return;
                }
            };

            self.enc_buf.drain(0..len + 2);
            if let Ok(msg) = decrypt(chunk.content(), &self.nonce_fetch_increment(), &self.precomputed_key) {
                self.try_deserialize(msg);
            }
        }
    }

    fn try_deserialize(&mut self, mut msg: Vec<u8>) {
        if self.input_remaining >= msg.len() {
            self.input_remaining -= msg.len();
        } else {
            self.input_remaining = 0;
        }

        self.dec_buf.append(&mut msg);

        if self.input_remaining == 0 {
            match PeerMessageResponse::from_bytes(self.dec_buf.clone()) {
                Ok(msg) => {
                    self.store_message(msg);
                    self.dec_buf.clear();
                }
                Err(BinaryReaderError::Underflow { bytes }) => self.input_remaining += bytes,
                Err(e) => {
                    log::error!("failed to decrypt message: {}", e);
                    self.dec_buf.clear();
                }
            }
        }
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.remote_nonce.increment();
        std::mem::replace(&mut self.remote_nonce, incremented)
    }

    fn store_message(&mut self, msg: PeerMessageResponse) {
        log::trace!("Message received: {:?}", msg);
    }
}