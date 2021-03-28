// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::{
    crypto_box::PrecomputedKey,
    CryptoError,
    nonce::{NoncePair, Nonce, generate_nonces},
};
use super::{Identity, common::Initiator};

#[derive(Clone)]
pub struct Keys {
    pub local: Key,
    pub remote: Key,
}

#[derive(Clone)]
pub struct Key {
    key: PrecomputedKey,
    nonce: Nonce,
}

impl Keys {
    pub fn new(
        identity: &Identity,
        local: &[u8],
        remote: &[u8],
        initiator: Initiator,
    ) -> Result<Self, CryptoError> {
        use crypto::crypto_box::CryptoKey;

        // check if the identity belong to one of the parties
        if identity.public_key.as_ref() != local[4..36].as_ref() {
            return Err(CryptoError::InvalidKey {
                reason: format!("The communication does not belong to the local node"),
            });
        };

        let pk = CryptoKey::from_bytes(&remote[4..36])?;
        let sk = CryptoKey::from_bytes(&identity.secret_key)?;

        let NoncePair { local, remote } =
            generate_nonces(local, remote, initiator.incoming()).unwrap();
        let key = PrecomputedKey::precompute(&pk, &sk);
        Ok(Keys {
            local: Key {
                key: key.clone(),
                nonce: local,
            },
            remote: Key { key, nonce: remote },
        })
    }
}

impl Key {
    pub fn decrypt(&mut self, payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let plain = self.key.decrypt(&payload[2..], &self.nonce)?;
        self.nonce = self.nonce.increment();
        Ok(plain)
    }
}
