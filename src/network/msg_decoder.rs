use crypto::{
    crypto_box::{PrecomputedKey, decrypt},
    nonce::Nonce,
};
use tezos_messages::p2p::{
    binary_message::BinaryChunk,
};
use crate::network::prelude::*;
use std::convert::TryFrom;
use bytes::Buf;
use rocksdb::DB;
use std::sync::Arc;
use std::time::Instant;

pub struct EncryptedMessageDecoder {
    db: Arc<DB>,
    precomputed_key: PrecomputedKey,
    remote_nonce: Nonce,
    peer_id: String,
    processing: bool,
    enc_buf: Vec<u8>,
    stamper: Instant,
}

impl EncryptedMessageDecoder {
    pub fn new(precomputed_key: PrecomputedKey, remote_nonce: Nonce, peer_id: String, db: Arc<DB>) -> Self {
        Self {
            db,
            precomputed_key,
            remote_nonce,
            peer_id,
            processing: false,
            enc_buf: Default::default(),
            stamper: Instant::now(),
        }
    }

    pub fn recv_msg(&mut self, enc: NetworkMessage) {
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
                let _ = self.db.put(self.stamper.elapsed().as_nanos().to_be_bytes(), &msg);
            }
        }
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.remote_nonce.increment();
        std::mem::replace(&mut self.remote_nonce, incremented)
    }

    fn store_message(&mut self) {}
}