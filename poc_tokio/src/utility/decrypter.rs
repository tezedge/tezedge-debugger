use bytes::Buf;
use tracing::{warn, error};
use crypto::{
    crypto_box::{PrecomputedKey, decrypt},
    nonce::Nonce,
};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::{
    encoding::{
        metadata::MetadataMessage,
        peer::PeerMessageResponse,
    },
    binary_message::{BinaryMessage, BinaryChunk},
};
use std::convert::TryFrom;
use crate::messages::prelude::*;

pub struct P2pDecrypter {
    precomputed_key: PrecomputedKey,
    nonce: Nonce,
    metadata: bool,
    inc_buf: Vec<u8>,
    dec_buf: Vec<u8>,
    input_remaining: usize,
}

impl P2pDecrypter {
    pub fn new(precomputed_key: PrecomputedKey, nonce: Nonce) -> Self {
        Self {
            precomputed_key,
            nonce,
            metadata: false,
            inc_buf: Default::default(),
            dec_buf: Default::default(),
            input_remaining: 0,
        }
    }

    pub fn recv_msg(&mut self, enc: &Packet) -> Option<Vec<PeerMessage>> {
        if enc.has_payload() {
            self.inc_buf.extend_from_slice(&enc.payload());

            if self.inc_buf.len() > 2 {
                if let Some(decrypted) = self.try_decrypt() {
                    return self.try_deserialize(decrypted);
                }
            }
        }
        None
    }

    fn try_decrypt(&mut self) -> Option<Vec<u8>> {
        let len = (&self.inc_buf[0..2]).get_u16() as usize;
        if self.inc_buf[2..].len() >= len {
            let chunk = match BinaryChunk::try_from(self.inc_buf[0..len + 2].to_vec()) {
                Ok(chunk) => chunk,
                Err(e) => {
                    error!(error = display(e), "failed to load binary chunk");
                    return None;
                }
            };

            self.inc_buf.drain(0..len + 2);
            match decrypt(chunk.content(), &self.nonce_fetch_increment(), &self.precomputed_key) {
                Ok(msg) => {
                    Some(msg)
                }
                Err(_) => { None }
            }
        } else {
            None
        }
    }

    fn try_deserialize(&mut self, mut msg: Vec<u8>) -> Option<Vec<PeerMessage>> {
        if !self.metadata {
            self.try_deserialize_meta(&mut msg)
        } else {
            self.try_deserialize_p2p(&mut msg)
        }
    }

    fn try_deserialize_meta(&mut self, msg: &mut Vec<u8>) -> Option<Vec<PeerMessage>> {
        self.input_remaining = self.input_remaining.saturating_sub(msg.len());
        self.dec_buf.append(msg);

        if self.input_remaining == 0 {
            loop {
                match MetadataMessage::from_bytes(self.dec_buf.clone()) {
                    Ok(msg) => {
                        self.dec_buf.clear();
                        self.metadata = true;
                        return Some(vec![msg.into()]);
                    }
                    Err(BinaryReaderError::Underflow { bytes }) => {
                        self.input_remaining += bytes;
                        return None;
                    }
                    Err(BinaryReaderError::Overflow { bytes }) => {
                        self.dec_buf.drain(self.dec_buf.len() - bytes..);
                    }
                    Err(e) => {
                        warn!(error = display(e), "failed to deserialize message");
                        return None;
                    }
                }
            }
        } else { None }
    }

    fn try_deserialize_p2p(&mut self, msg: &mut Vec<u8>) -> Option<Vec<PeerMessage>> {
        self.input_remaining = self.input_remaining.saturating_sub(msg.len());
        self.dec_buf.append(msg);

        if self.input_remaining == 0 {
            loop {
                match PeerMessageResponse::from_bytes(self.dec_buf.clone()) {
                    Ok(msg) => {
                        self.dec_buf.clear();
                        return if msg.messages().len() == 0 {
                            None
                        } else {
                            // msg.messages().iter(|x| x.into()).collect()
                            Some(msg.messages().iter().map(|x| x.clone().into()).collect())
                        };
                    }
                    Err(BinaryReaderError::Underflow { bytes }) => {
                        self.input_remaining += bytes;
                        return None;
                    }
                    Err(BinaryReaderError::Overflow { bytes }) => {
                        self.dec_buf.drain(self.dec_buf.len() - bytes..);
                    }
                    Err(e) => {
                        warn!(error = display(e), "failed to deserialize message");
                        return None;
                    }
                }
            }
        } else { None }
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce.increment();
        std::mem::replace(&mut self.nonce, incremented)
    }
}