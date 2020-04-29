// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
/// UNUSED
use crypto::crypto_box::{PrecomputedKey, encrypt};
use crypto::nonce::Nonce;
use failure::Error;

pub struct EncryptedMessageEncoder {
    precomputed_key: PrecomputedKey,
    local_nonce: Nonce,
}

impl EncryptedMessageEncoder {
    pub fn new(precomputed_key: PrecomputedKey, local_nonce: Nonce) -> Self {
        Self {
            precomputed_key,
            local_nonce,
        }
    }

    fn try_encrypt(&mut self, msg: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(encrypt(msg, &self.nonce_fetch_increment(), &self.precomputed_key)?)
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.local_nonce.increment();
        std::mem::replace(&mut self.local_nonce, incremented)
    }
}