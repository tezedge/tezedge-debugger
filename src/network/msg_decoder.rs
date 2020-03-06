use crypto::{
    crypto_box::{PrecomputedKey, decrypt},
    nonce::Nonce,
};
use tezos_messages::p2p::{
    binary_message::{BinaryMessage, BinaryChunk},
    encoding::peer::PeerMessageResponse,
};
use tezos_encoding::binary_reader::BinaryReaderError;
use crate::network::prelude::*;

pub struct EncryptedMessageDecoder {
    precomputed_key: PrecomputedKey,
    remote_nonce: Nonce,
    peer_id: String,
    input_remaining: usize,
}

impl EncryptedMessageDecoder {
    pub fn new(precomputed_key: PrecomputedKey, remote_nonce: Nonce, peer_id: String) -> Self {
        Self {
            precomputed_key,
            remote_nonce,
            peer_id,
            input_remaining: 0,
        }
    }

    pub fn recv_msg(&mut self, enc: NetworkMessage) {
        let mut input_data = vec![];

        let chunk = BinaryChunk::from_content(enc.raw_msg()).unwrap();
        match decrypt(chunk.content(), &self.nonce_fetch_increment(), &self.precomputed_key) {
            Ok(message_decrypted) => {
                if self.input_remaining >= message_decrypted.len() {
                    self.input_remaining -= message_decrypted.len();
                } else {
                    self.input_remaining = 0;
                }

                input_data.extend(enc.raw_msg());

                if self.input_remaining == 0 {
                    match PeerMessageResponse::from_bytes(input_data.clone()) {
                        Ok(message) => log::info!("-- Decrypted new message message: {:?}", message),
                        Err(BinaryReaderError::Underflow { bytes }) => self.input_remaining += bytes,
                        Err(e) => log::warn!("Failed to deserialize message: {}", e),
                    }
                }
            }
            Err(error) => {
                log::warn!("Failed to deserialize message: {}", error);
            }
        }
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.remote_nonce.increment();
        std::mem::replace(&mut self.remote_nonce, incremented)
    }
}