// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{rc::Rc, cell::RefCell};
use typenum::Bit;
use either::Either;
use super::{
    state::{Initial, HaveCm, Uncertain, HaveKey, HaveNotKey, CannotDecrypt, MakeKeyOutput},
    tables::{connection, chunk},
    common::{Local, Remote},
    Identity,
};

pub struct Handshake {
    local: Half<Local>,
    remote: Half<Remote>,
}

enum Half<S> {
    Initial(Initial<S>),
    HaveCm(HaveCm<S>),
}

pub struct HandshakeOutput {
    pub local: HandshakeDone<Local>,
    pub l_chunk: Option<chunk::Item>,
    pub remote: HandshakeDone<Remote>,
    pub r_chunk: Option<chunk::Item>,
}

impl From<MakeKeyOutput> for HandshakeOutput {
    fn from(v: MakeKeyOutput) -> Self {
        HandshakeOutput {
            local: v.local.into(),
            l_chunk: v.l_chunk,
            remote: v.remote.into(),
            r_chunk: v.r_chunk,
        }
    }
}

impl Handshake {
    pub fn new(cn: Rc<RefCell<connection::Item>>, id: Identity) -> Self {
        let local = Half::Initial(Initial::new(cn.clone(), id.clone()));
        let remote = Half::Initial(Initial::new(cn, id));
        Handshake { local, remote }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Handshake { local: Half::Initial(l), remote: Half::Initial(r) } => {
                l.is_empty() && r.is_empty()
            },
            _ => false,
        }
    }

    fn initial(l: Initial<Local>, r: Initial<Remote>) -> Self {
        Handshake {
            local: Half::Initial(l),
            remote: Half::Initial(r),
        }
    }

    fn local_cm(l: HaveCm<Local>, r: Initial<Remote>) -> Self {
        Handshake {
            local: Half::HaveCm(l),
            remote: Half::Initial(r),
        }
    }

    fn remote_cm(l: Initial<Local>, r: HaveCm<Remote>) -> Self {
        Handshake {
            local: Half::Initial(l),
            remote: Half::HaveCm(r),
        }
    }

    pub fn handle_data(self, payload: &[u8], net: bool, incoming: bool) -> Either<Self, HandshakeOutput> {
        match self {
            Handshake {
                local: Half::Initial(l),
                remote: Half::Initial(r),
            } => {
                if !incoming {
                    match l.handle_data(payload) {
                        Either::Left(l) => Either::Left(Handshake::initial(l, r)),
                        Either::Right(l) => Either::Left(Handshake::local_cm(l, r)),
                    }
                } else {
                    match r.handle_data(payload) {
                        Either::Left(r) => Either::Left(Handshake::initial(l, r)),
                        Either::Right(r) => Either::Left(Handshake::remote_cm(l, r)),
                    }
                }
            },
            Handshake {
                local: Half::HaveCm(l),
                remote: Half::Initial(r),
            } => {
                if !incoming {
                    match l.handle_data(payload) {
                        Ok(l) => Either::Left(Handshake::local_cm(l, r)),
                        Err((l, l_chunk)) => {
                            let (r, mut r_chunk) = r.uncertain();
                            if let Some(c) = &mut r_chunk {
                                c.net(net);
                            }
                            Either::Right(HandshakeOutput {
                                local: HandshakeDone::Uncertain(l),
                                l_chunk,
                                remote: HandshakeDone::Uncertain(r),
                                r_chunk,
                            })
                        },
                    }
                } else {
                    match r.handle_data(payload) {
                        Either::Left(r) => Either::Left(Handshake::local_cm(l, r)),
                        Either::Right(r) => Either::Right(l.make_key(r).into()),
                    }
                }
            },
            Handshake {
                local: Half::Initial(l),
                remote: Half::HaveCm(r),
            } => {
                if incoming {
                    match r.handle_data(payload) {
                        Ok(r) => Either::Left(Handshake::remote_cm(l, r)),
                        Err((r, r_chunk)) => {
                            let (l, mut l_chunk) = l.uncertain();
                            if let Some(c) = &mut l_chunk {
                                c.net(net);
                            }
                            Either::Right(HandshakeOutput {
                                local: HandshakeDone::Uncertain(l),
                                l_chunk,
                                remote: HandshakeDone::Uncertain(r),
                                r_chunk,
                            })
                        },
                    }
                } else {
                    match l.handle_data(payload) {
                        Either::Left(l) => Either::Left(Handshake::remote_cm(l, r)),
                        Either::Right(l) => Either::Right(l.make_key(r).into()),
                    }
                }
            },
            _ => panic!(),
        }
    }
}

pub enum HandshakeDone<S>
where
    S: Bit,
{
    Uncertain(Uncertain<S>),
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
    pub fn handle_data<H>(self, payload: &[u8], net: bool, handler: &mut H) -> Self
    where
        H: ChunkHandler,
    {
        match self {
            HandshakeDone::Uncertain(mut state) => {
                let mut chunk = state.handle_data(payload);
                chunk.net(net);
                handler.handle_chunk(chunk);
                HandshakeDone::Uncertain(state)
            },
            HandshakeDone::HaveKey(state) => {
                let mut temp_state = state.handle_data(payload);
                while let Some(mut chunk) = temp_state.next() {
                    chunk.net(net);
                    handler.handle_chunk(chunk)
                }
                match temp_state.over() {
                    Ok(state) => HandshakeDone::HaveKey(state),
                    Err(state) => {
                        handler.update_cn();
                        HandshakeDone::CannotDecrypt(state)
                    },
                }
            },
            HandshakeDone::HaveNotKey(mut state) => {
                let mut chunk = state.handle_data(payload);
                chunk.net(net);
                handler.handle_chunk(chunk);
                HandshakeDone::HaveNotKey(state)
            },
            HandshakeDone::CannotDecrypt(mut state) => {
                let mut chunk = state.handle_data(payload);
                chunk.net(net);
                handler.handle_chunk(chunk);
                HandshakeDone::CannotDecrypt(state)
            },
        }
    }
}

pub trait ChunkHandler {
    fn handle_chunk(&mut self, chunk: chunk::Item);
    fn update_cn(&mut self);
}
