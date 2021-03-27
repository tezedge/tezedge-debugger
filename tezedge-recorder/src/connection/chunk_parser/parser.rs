// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use typenum::Bit;
use either::Either;
use super::{
    state::{Initial, HaveCm, HaveKey, HaveNotKey, CannotDecrypt, MakeKeyOutput},
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

impl From<MakeKeyOutput> for HandshakeOutput {
    fn from(v: MakeKeyOutput) -> Self {
        HandshakeOutput {
            cn: v.cn,
            local: v.local.into(),
            l_chunk: v.l_chunk,
            remote: v.remote.into(),
            r_chunk: v.r_chunk,
        }
    }
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
            Handshake { cn, id, local: Either::Right(l), remote: Either::Left(r) } => {
                if !incoming {
                    Either::Left(Handshake::local_cm(cn, id, l.handle_data(payload), r))
                } else {
                    match r.handle_data(payload) {
                        Either::Left(r) => Either::Left(Handshake::local_cm(cn, id, l, r)),
                        Either::Right(r) => Either::Right(l.make_key(r, &id, cn).into()),
                    }
                }
            },
            Handshake { cn, id, local: Either::Left(l), remote: Either::Right(r) } => {
                if incoming {
                    Either::Left(Handshake::remote_cm(cn, id, l, r.handle_data(payload)))
                } else {
                    match l.handle_data(payload) {
                        Either::Left(l) => Either::Left(Handshake::remote_cm(cn, id, l, r)),
                        Either::Right(l) => Either::Right(l.make_key(r, &id, cn).into()),
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

impl<S> From<Result<HaveKey<S>, HaveNotKey<S>>> for HandshakeDone<S>
where
    S: Bit,
{
    fn from(v: Result<HaveKey<S>, HaveNotKey<S>>) -> Self {
        match v {
            Ok(x) => HandshakeDone::HaveKey(x),
            Err(x) => HandshakeDone::HaveNotKey(x),
        }
    }
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
