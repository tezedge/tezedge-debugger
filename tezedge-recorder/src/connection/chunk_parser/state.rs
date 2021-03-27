// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;
use either::Either;
use thiserror::Error;
use crypto::{proof_of_work, blake2b::Blake2bError, hash::FromBytesError};
use typenum::{self, Bit};
use super::{
    buffer::Buffer,
    key::{Keys, Key},
    tables::{connection, chunk},
    common::{Sender, Local, Remote},
    Identity,
};

struct Inner<S>
where
    S: Bit,
{
    connection_id: u128,
    buffer: Buffer,
    incoming: PhantomData<S>,
}

impl<S> Inner<S>
where
    S: Bit,
{
    fn chunk(&self, counter: u64, bytes: Vec<u8>, plain: Vec<u8>) -> chunk::Item {
        chunk::Item::new(self.connection_id, Sender::new(S::BOOL), counter, bytes, plain)
    }
}

/// State machine:
///
///               Handshake
///     Handshake             HaveCm
///                 HaveNotKey      HaveKey
///            HaveNotKey                  HaveData
///                         CannotDecrypt   HaveKey   HaveData
///                       CannotDecrypt

pub struct Initial<S>
where
    S: Bit,
{
    inner: Inner<S>,
}

pub struct HaveCm<S>
where
    S: Bit,
{
    inner: Inner<S>,
}

pub struct HaveKey<S>
where
    S: Bit,
{
    inner: Inner<S>,
    key: Key,
}

pub struct HaveNotKey<S>
where
    S: Bit,
{
    inner: Inner<S>,
}

pub struct HaveData<S>
where
    S: Bit,
{
    inner: Inner<S>,
    key: Key,
    error: bool,
}

pub struct CannotDecrypt<S>
where
    S: Bit,
{
    inner: Inner<S>,
}

impl<S> Initial<S>
where
    S: Bit,
{
    pub fn new(connection_id: u128) -> Self {
        Initial {
            inner: Inner {
                connection_id,
                buffer: Buffer::default(),
                incoming: PhantomData,
            },    
        }
    }

    pub fn handle_data(mut self, payload: &[u8]) -> Either<Self, HaveCm<S>> {
        self.inner.buffer.handle_data(payload);
        if self.inner.buffer.have_chunk().is_some() {
            Either::Right(HaveCm { inner: self.inner })
        } else {
            Either::Left(self)
        }
    }
}

impl<S> HaveCm<S>
where
    S: Bit,
{
    /// Should not need to call
    pub fn handle_data(mut self, payload: &[u8]) -> Self {
        self.inner.buffer.handle_data(payload);
        self
    }

    fn have_key(mut self, key: Key) -> (HaveKey<S>, chunk::Item) {
        let (counter, bytes) = self.inner.buffer.next().unwrap();
        let remaining = self.inner.buffer.remaining();
        if remaining > 0 {
            log::warn!(
                "have {} bytes after connection message received, but before got key",
                remaining,
            );
        }
        let plain = bytes[2..].to_vec();
        let c = self.inner.chunk(counter, bytes, plain);
        (
            HaveKey {
                inner: self.inner,
                key,
            },
            c,
        )
    }

    fn have_not_key(mut self) -> (HaveNotKey<S>, chunk::Item) {
        let (counter, bytes) = self.inner.buffer.cleanup();
        let c = self.inner.chunk(counter, bytes, Vec::new());
        (
            HaveNotKey {
                inner: self.inner,
            },
            c,
        )
    }
}

pub struct MakeKeyOutput {
    pub cn: connection::Item,
    pub local: Result<HaveKey<Local>, HaveNotKey<Local>>,
    pub l_chunk: chunk::Item,
    pub remote: Result<HaveKey<Remote>, HaveNotKey<Remote>>,
    pub r_chunk: chunk::Item,
}

impl HaveCm<Local> {
    pub fn make_key(
        self,
        peer: HaveCm<Remote>,
        identity: &Identity,
        mut cn: connection::Item,
    ) -> MakeKeyOutput {
        #[derive(Error, Debug)]
        pub enum HandshakeWarning {
            #[error("connection message is too short {}", _0)]
            ConnectionMessageTooShort(usize),
            #[error("proof-of-work check failed")]
            PowInvalid(f64),
            #[error("cannot calc peer_id: black2b hashing error {}", _0)]
            Blake2b(Blake2bError),
            #[error("cannot calc peer_id: from bytes error {}", _0)]
            FromBytes(FromBytesError),
        }

        let check = |payload: &[u8]| -> Result<String, HandshakeWarning> {
            use crypto::{blake2b, hash::HashType};

            if payload.len() <= 88 {
                return Err(HandshakeWarning::ConnectionMessageTooShort(payload.len()));
            }
            // TODO: move to config
            let target = 26.0;
            if proof_of_work::check_proof_of_work(&payload[4..60], target).is_err() {
                return Err(HandshakeWarning::PowInvalid(target));
            }

            let hash = blake2b::digest_128(&payload[4..36]).map_err(HandshakeWarning::Blake2b)?;
            HashType::CryptoboxPublicKeyHash
                .hash_to_b58check(&hash)
                .map_err(HandshakeWarning::FromBytes)
        };

        let local_chunk = self.inner.buffer.have_chunk().unwrap();
        let remote_chunk = peer.inner.buffer.have_chunk().unwrap();
        match Keys::new(identity, local_chunk, remote_chunk, cn.initiator.clone()) {
            Ok(Keys { local, remote }) => {
                let (l, l_chunk) = self.have_key(local);
                let (r, r_chunk) = peer.have_key(remote);
                match check(&l_chunk.bytes) {
                    Ok(peer_id) => if peer_id != identity.peer_id {
                        cn.add_comment("local peer id does not match".to_string())
                    },
                    Err(warning) => cn.add_comment(format!("Outgoing: {}", warning)),
                }
                match check(&r_chunk.bytes) {
                    Ok(peer_id) => cn.set_peer_id(peer_id),
                    Err(warning) => cn.add_comment(format!("Incoming: {}", warning)),
                }
                MakeKeyOutput { cn, local: Ok(l), l_chunk, remote: Ok(r), r_chunk }
            },
            Err(error) => {
                let (l, l_chunk) = self.have_not_key();
                let (r, r_chunk) = peer.have_not_key();
                cn.add_comment(format!("Key calculate error: {}", error));
                MakeKeyOutput { cn, local: Err(l), l_chunk, remote: Err(r), r_chunk }
            },
        }
    }
}

impl<S> HaveNotKey<S>
where
    S: Bit,
{
    pub fn handle_data(&mut self, payload: &[u8]) -> chunk::Item {
        self.inner.buffer.handle_data(payload);
        let (counter, bytes) = self.inner.buffer.cleanup();
        self.inner.chunk(counter, bytes, Vec::new())
    }
}

impl<S> HaveKey<S>
where
    S: Bit,
{
    pub fn handle_data(mut self, payload: &[u8]) -> HaveData<S> {
        self.inner.buffer.handle_data(payload);
        HaveData {
            inner: self.inner,
            key: self.key,
            error: false,
        }
    }
}

impl<S> Iterator for HaveData<S>
where
    S: Bit,
{
    type Item = chunk::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.error {
            return None;
        }
        let _ = self.inner.buffer.have_chunk()?;
        let (counter, bytes) = self.inner.buffer.next().unwrap();
        match self.key.decrypt(&bytes) {
            Ok(plain) => {
                Some(self.inner.chunk(counter, bytes, plain))
            },
            Err(_) => {
                self.error = true;
                let id = self.inner.connection_id;
                log::warn!("cannot decrypt: {}, {}, {:?}", id, counter, Sender::new(S::BOOL));
                let (counter, bytes) = self.inner.buffer.cleanup();
                Some(self.inner.chunk(counter, bytes, Vec::new()))
            },
        }
    }
}

impl<S> HaveData<S>
where
    S: Bit,
{
    pub fn over(self) -> Result<HaveKey<S>, CannotDecrypt<S>> {
        if self.error {
            Err(CannotDecrypt {
                inner: self.inner,
            })
        } else {
            Ok(HaveKey {
                inner: self.inner,
                key: self.key,
            })
        }
    }
}

impl<S> CannotDecrypt<S>
where
    S: Bit,
{
    pub fn handle_data(&mut self, payload: &[u8]) -> chunk::Item {
        self.inner.buffer.handle_data(payload);
        let (counter, bytes) = self.inner.buffer.cleanup();
        self.inner.chunk(counter, bytes, Vec::new())
    }
}
