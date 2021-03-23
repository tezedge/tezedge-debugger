// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::{crypto_box::PrecomputedKey, CryptoError, nonce::{NoncePair, Nonce, generate_nonces}};
use super::Identity;

#[derive(Clone)]
pub struct Key {
    key: PrecomputedKey,
    local: Nonce,
    remote: Nonce,
}

impl Key {
    pub fn new(identity: &Identity, initiator: &[u8], responder: &[u8]) -> Result<Self, CryptoError> {
        use crypto::crypto_box::CryptoKey;

        // check if the identity belong to one of the parties
        let (pk, incoming) = if initiator[4..36] == identity.public_key {
            (&responder[4..36], false)
        } else if responder[4..36] == identity.public_key {
            (&initiator[4..36], true)
        } else {
            return Err(CryptoError::InvalidKey {
                reason: format!("The communication does not belong to {}", identity.peer_id),
            });
        };
        let pk = CryptoKey::from_bytes(pk)?;
        let sk = CryptoKey::from_bytes(&identity.secret_key)?;

        let sent_msg = if incoming { responder } else { initiator };
        let recv_msg = if incoming { initiator } else { responder };
        let NoncePair { local, remote } = generate_nonces(sent_msg, recv_msg, incoming).unwrap();
        Ok(Key {
            key: PrecomputedKey::precompute(&pk, &sk),
            local,
            remote
        })
    }

    pub fn decrypt(&mut self, payload: &[u8], incoming: bool) -> Result<Vec<u8>, CryptoError> {
        if incoming {
            let plain = self.key.decrypt(&payload, &self.remote)?;
            self.remote = self.remote.increment();
            Ok(plain)
        } else {
            let plain = self.key.decrypt(&payload, &self.local)?;
            self.local = self.local.increment();
            Ok(plain)
        }
    }
}
