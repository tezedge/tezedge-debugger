// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use typenum::Bit;
use either::Either;
use thiserror::Error;
use crypto::{proof_of_work, blake2b::Blake2bError, hash::FromBytesError};
use super::{
    state::{Initial, HaveCm, HaveKey, HaveNotKey, CannotDecrypt},
    tables::{connection, chunk},
    common::{Local, Remote},
    Identity,
};

pub struct Handshake {
    cn: connection::Item,
    id: Identity,
    local: Either<Initial<Local>, HaveCm<Local>>,
    remote: Either<Initial<Remote>, HaveCm<Remote>>,
}

pub struct HandshakeOutput {
    pub cn: connection::Item,
    pub local: HandshakeDone<Local>,
    pub l_chunk: chunk::Item,
    pub remote: HandshakeDone<Remote>,
    pub r_chunk: chunk::Item,
}

impl Handshake {
    pub fn new(cn: connection::Item, id: Identity) -> Self {
        let local = Either::Left(Initial::new(cn.id));
        let remote = Either::Left(Initial::new(cn.id));
        Handshake { cn, id, local, remote }
    }

    fn initial(cn: connection::Item, id: Identity, l: Initial<Local>, r: Initial<Remote>) -> Self {
        Handshake {
            cn,
            id,
            local: Either::Left(l),
            remote: Either::Left(r),
        }
    }

    fn local_cm(cn: connection::Item, id: Identity, l: HaveCm<Local>, r: Initial<Remote>) -> Self {
        Handshake {
            cn,
            id,
            local: Either::Right(l),
            remote: Either::Left(r),
        }
    }

    fn remote_cm(cn: connection::Item, id: Identity, l: Initial<Local>, r: HaveCm<Remote>) -> Self {
        Handshake {
            cn,
            id,
            local: Either::Left(l),
            remote: Either::Right(r),
        }
    }

    pub fn handle_data(
        self,
        payload: &[u8],
        incoming: bool,
    ) -> Either<Self, HandshakeOutput> {
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

        match self {
            Handshake { cn, id, local: Either::Left(l), remote: Either::Left(r) } => {
                if !incoming {
                    match l.handle_data(payload) {
                        Either::Left(l) => Either::Left(Handshake::initial(cn, id, l, r)),
                        Either::Right(l) => Either::Left(Handshake::local_cm(cn, id, l, r)),
                    }
                } else {
                    match r.handle_data(payload) {
                        Either::Left(r) => Either::Left(Handshake::initial(cn, id, l, r)),
                        Either::Right(r) => Either::Left(Handshake::remote_cm(cn, id, l, r)),
                    }
                }
            },
            Handshake { mut cn, id, local: Either::Right(l), remote: Either::Left(r) } => {
                if !incoming {
                    Either::Left(Handshake::local_cm(cn, id, l.handle_data(payload), r))
                } else {
                    match r.handle_data(payload) {
                        Either::Left(r) => Either::Left(Handshake::local_cm(cn, id, l, r)),
                        Either::Right(r) => {
                            match l.make_key(r, &id, &cn.initiator) {
                                Ok(((ls, l_chunk), (rs, r_chunk))) => {
                                    match check(&l_chunk.bytes) {
                                        Ok(peer_id) => if peer_id != id.peer_id {
                                            cn.add_comment("local peer id does not match".to_string())
                                        },
                                        Err(warning) => cn.add_comment(format!("Outgoing: {}", warning)),
                                    }
                                    match check(&r_chunk.bytes) {
                                        Ok(peer_id) => cn.set_peer_id(peer_id),
                                        Err(warning) => cn.add_comment(format!("Incoming: {}", warning)),
                                    }
                                    Either::Right(HandshakeOutput {
                                        cn,
                                        local: HandshakeDone::HaveKey(ls),
                                        l_chunk,
                                        remote: HandshakeDone::HaveKey(rs),
                                        r_chunk,
                                    })
                                },
                                Err(((ls, l_chunk), (rs, r_chunk), error)) => {
                                    cn.add_comment(format!("Key calculate error: {}", error));
                                    Either::Right(HandshakeOutput {
                                        cn,
                                        local: HandshakeDone::HaveNotKey(ls),
                                        l_chunk,
                                        remote: HandshakeDone::HaveNotKey(rs),
                                        r_chunk,
                                    })
                                },
                            }
                        },
                    }
                }
            },
            Handshake { mut cn, id, local: Either::Left(l), remote: Either::Right(r) } => {
                if incoming {
                    Either::Left(Handshake::remote_cm(cn, id, l, r.handle_data(payload)))
                } else {
                    match l.handle_data(payload) {
                        Either::Left(l) => Either::Left(Handshake::remote_cm(cn, id, l, r)),
                        Either::Right(l) => {
                            match l.make_key(r, &id, &cn.initiator) {
                                Ok(((ls, l_chunk), (rs, r_chunk))) => {
                                    match check(&l_chunk.bytes) {
                                        Ok(peer_id) => if peer_id != id.peer_id {
                                            cn.add_comment("local peer id does not match".to_string())
                                        },
                                        Err(warning) => cn.add_comment(format!("Outgoing: {}", warning)),
                                    }
                                    match check(&r_chunk.bytes) {
                                        Ok(peer_id) => cn.set_peer_id(peer_id),
                                        Err(warning) => cn.add_comment(format!("Incoming: {}", warning)),
                                    }
                                    Either::Right(HandshakeOutput {
                                        cn,
                                        local: HandshakeDone::HaveKey(ls),
                                        l_chunk,
                                        remote: HandshakeDone::HaveKey(rs),
                                        r_chunk,
                                    })
                                },
                                Err(((ls, l_chunk), (rs, r_chunk), error)) => {
                                    cn.add_comment(format!("Key calculate error: {}", error));
                                    Either::Right(HandshakeOutput {
                                        cn,
                                        local: HandshakeDone::HaveNotKey(ls),
                                        l_chunk,
                                        remote: HandshakeDone::HaveNotKey(rs),
                                        r_chunk,
                                    })
                                },
                            }
                        },
                    }
                }
            },
            Handshake { local: Either::Right(_), remote: Either::Right(_), .. } => panic!(),
        }
    }
}

pub enum HandshakeDone<S>
where
    S: Bit,
{
    HaveKey(HaveKey<S>),
    HaveNotKey(HaveNotKey<S>),
    CannotDecrypt(CannotDecrypt<S>),
}

impl<S> HandshakeDone<S>
where
    S: Bit,
{
    pub fn handle_data<H>(self, payload: &[u8], handler: &mut H) -> Self
    where
        H: ChunkHandler,
    {
        match self {
            HandshakeDone::HaveKey(state) => {
                let mut temp_state = state.handle_data(payload);
                while let Some(chunk) = temp_state.next() {
                    handler.handle_chunk(chunk)
                }
                match temp_state.over() {
                    Ok(state) => HandshakeDone::HaveKey(state),
                    Err(state) => HandshakeDone::CannotDecrypt(state),
                }
            },
            HandshakeDone::HaveNotKey(mut state) => {
                handler.handle_chunk(state.handle_data(payload));
                HandshakeDone::HaveNotKey(state)
            },
            HandshakeDone::CannotDecrypt(mut state) => {
                handler.handle_chunk(state.handle_data(payload));
                HandshakeDone::CannotDecrypt(state)
            }
        }
    }
}

pub trait ChunkHandler {
    fn handle_chunk(&mut self, chunk: chunk::Item);
}

impl<F> ChunkHandler for F
where
    F: FnMut(chunk::Item),
{
    fn handle_chunk(&mut self, chunk: chunk::Item) {
        self(chunk)
    }
}
