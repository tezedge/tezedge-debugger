// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc, time::Duration};
use either::Either;
use super::{
    chunk_parser::{Handshake, HandshakeOutput, HandshakeDone, ChunkHandler},
    message_parser::MessageParser,
    Identity, Database,
    common::{Local, Remote, Initiator},
    tables::{syscall, connection},
};

pub struct Connection<Db> {
    state: Option<ConnectionState<Db>>,
    item: connection::Item,
    incoming_offset: u64,
    outgoing_offset: u64,
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
    pub fn new(
        timestamp: Duration,
        remote_addr: SocketAddr,
        incoming: bool,
        identity: Identity,
        db: Arc<Db>,
    ) -> Self {
        let item = connection::Item::new(Initiator::new(incoming), remote_addr, timestamp);
        db.store_connection(item.clone());
        let syscall_item = syscall::Item {
            cn_id: item.key(),
            timestamp,
            inner: if incoming {
                syscall::ItemInner::Accept(Ok(()))
            } else {
                syscall::ItemInner::Connect(Ok(()))
            }
        };
        db.store_syscall(syscall_item);

        let state = ConnectionState::Handshake(Handshake::new(&item.key(), identity));
        Connection {
            state: Some(state),
            item,
            incoming_offset: 0,
            outgoing_offset: 0,
            db,
        }
    }

    pub fn cn_id(&self) -> connection::Key {
        self.item.key()
    }

    pub fn handle_data(&mut self, timestamp: Duration, payload: &[u8], net: bool, incoming: bool) {
        let offset;
        if incoming {
            offset = self.incoming_offset;
            self.incoming_offset += payload.len() as u64;
        } else {
            offset = self.outgoing_offset;
            self.outgoing_offset += payload.len() as u64;
        }
        let data_ref = Ok(syscall::DataRef {
            offset,
            length: payload.len() as u32,
        });
        let syscall_item = syscall::Item {
            cn_id: self.cn_id(),
            timestamp,
            inner: if incoming {
                syscall::ItemInner::Read(data_ref)
            } else {
                syscall::ItemInner::Write(data_ref)
            }
        };
        self.db.store_syscall(syscall_item);

        let state = match self.state.take().unwrap() {
            ConnectionState::Handshake(h) => {
                match h.handle_data(payload, net, incoming, &mut self.item) {
                    Either::Left(h) => ConnectionState::Handshake(h),
                    Either::Right(HandshakeOutput {
                        local,
                        l_chunk,
                        remote,
                        r_chunk,
                    }) => {
                        let mut local_mp = MessageParser::new(self.db.clone());
                        let mut remote_mp = MessageParser::new(self.db.clone());
                        self.db.update_connection(self.item.clone());
                        if let Some(chunk) = l_chunk {
                            local_mp.handle_chunk(chunk, &mut self.item);
                        }
                        if let Some(chunk) = r_chunk {
                            remote_mp.handle_chunk(chunk, &mut self.item);
                        }
                        ConnectionState::HandshakeDone {
                            local,
                            local_mp,
                            remote,
                            remote_mp,
                        }
                    },
                }
            },
            ConnectionState::HandshakeDone {
                local,
                mut local_mp,
                remote,
                mut remote_mp,
            } => {
                if !incoming {
                    ConnectionState::HandshakeDone {
                        local: local.handle_data(payload, net, &mut self.item, &mut local_mp),
                        local_mp,
                        remote,
                        remote_mp,
                    }
                } else {
                    ConnectionState::HandshakeDone {
                        local,
                        local_mp,
                        remote: remote.handle_data(payload, net, &mut self.item, &mut remote_mp),
                        remote_mp,
                    }
                }
            },
        };
        self.state = Some(state);
    }

    pub fn warn_fd_changed(&self) {
        if !matches!(&self.state, &Some(ConnectionState::Handshake(ref h)) if h.is_empty()) {
            log::warn!(
                "fd of: {} has took for other file, the connection is closed",
                self.item.remote_addr
            );
        }
    }

    pub fn join(self, timestamp: Duration) {
        let _ = timestamp;
        self.db.store_syscall(syscall::Item {
            cn_id: self.cn_id(),
            timestamp,
            inner: syscall::ItemInner::Close,
        });
    }
}
