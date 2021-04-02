// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc, rc::Rc, cell::RefCell};
use either::Either;
use super::{
    chunk_parser::{Handshake, HandshakeOutput, HandshakeDone, ChunkHandler},
    message_parser::MessageParser,
    Identity, Database,
    common::{Local, Remote, Initiator},
    tables::connection,
};

pub struct Connection<Db> {
    state: Option<ConnectionState<Db>>,
    item: Rc<RefCell<connection::Item>>,
    db: Arc<Db>,
}

#[allow(clippy::large_enum_variant)]
enum ConnectionState<Db> {
    Handshake(Handshake),
    HandshakeDone {
        local: HandshakeDone<Local>,
        local_mp: MessageParser<Db>,
        remote: HandshakeDone<Remote>,
        remote_mp: MessageParser<Db>,
    },
}

impl<Db> Connection<Db>
where
    Db: Database,
{
    pub fn new(remote_addr: SocketAddr, incoming: bool, identity: Identity, db: Arc<Db>) -> Self {
        let item = connection::Item::new(Initiator::new(incoming), remote_addr);
        let item = Rc::new(RefCell::new(item));
        Connection {
            state: Some(ConnectionState::Handshake(Handshake::new(item.clone(), identity))),
            item,
            db,
        }
    }

    pub fn handle_data(&mut self, payload: &[u8], net: bool, incoming: bool) {
        let state = match self.state.take().unwrap() {
            ConnectionState::Handshake(h) => match h.handle_data(payload, net, incoming) {
                Either::Left(h) => ConnectionState::Handshake(h),
                Either::Right(HandshakeOutput {
                    local,
                    l_chunk,
                    remote,
                    r_chunk,
                }) => {
                    let mut local_mp = MessageParser::new(self.item.clone(), self.db.clone());
                    let mut remote_mp = MessageParser::new(self.item.clone(), self.db.clone());
                    self.db.store_connection(self.item.borrow().clone());
                    if let Some(chunk) = l_chunk {
                        local_mp.handle_chunk(chunk);
                    }
                    if let Some(chunk) = r_chunk {
                        remote_mp.handle_chunk(chunk);
                    }
                    ConnectionState::HandshakeDone {
                        local,
                        local_mp,
                        remote,
                        remote_mp,
                    }
                },
            },
            ConnectionState::HandshakeDone {
                local,
                mut local_mp,
                remote,
                mut remote_mp,
            } => {
                if !incoming {
                    ConnectionState::HandshakeDone {
                        local: local.handle_data(payload, net, &mut local_mp),
                        local_mp,
                        remote,
                        remote_mp,
                    }
                } else {
                    ConnectionState::HandshakeDone {
                        local,
                        local_mp,
                        remote: remote.handle_data(payload, net, &mut remote_mp),
                        remote_mp,
                    }
                }
            },
        };
        self.state = Some(state);
    }

    pub fn warn_fd_changed(&self) {
        if !matches!(&self.state, &Some(ConnectionState::Handshake(ref h)) if h.is_empty()) {
            log::warn!("fd of: {} has took for other file, the connection is closed", self.item.borrow().remote_addr);
        }
    }

    pub fn join(self) {}
}
