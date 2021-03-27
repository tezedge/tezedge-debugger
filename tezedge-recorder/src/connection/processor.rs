// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc};
use either::Either;
use super::{
    chunk_parser::{Handshake, HandshakeOutput, HandshakeDone, ChunkHandler},
    message_parser::MessageParser,
    Identity, Database,
    common::{Local, Remote, Initiator},
    tables::connection,
};

pub struct Connection<Db> {
    chunk_parser: ChunkParser<Db>,
    db: Arc<Db>,
}

#[allow(clippy::large_enum_variant)]
enum ChunkParser<Db> {
    Invalid,
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
        use std::time::{SystemTime, UNIX_EPOCH};

        let connection_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_item = connection::Item::new(connection_id, Initiator::new(incoming), remote_addr);
        Connection {
            chunk_parser: ChunkParser::Handshake(Handshake::new(db_item, identity)),
            db,
        }
    }

    pub fn handle_data(&mut self, payload: &[u8], incoming: bool) {
        use std::mem;

        let parser = mem::replace(&mut self.chunk_parser, ChunkParser::Invalid);
        let parser = match parser {
            ChunkParser::Invalid => ChunkParser::Invalid,
            ChunkParser::Handshake(h) => match h.handle_data(payload, incoming) {
                Either::Left(h) => ChunkParser::Handshake(h),
                Either::Right(HandshakeOutput {
                    cn,
                    local,
                    l_chunk,
                    remote,
                    r_chunk,
                }) => {
                    let mut local_mp = MessageParser::new(cn.clone(), self.db.clone());
                    let mut remote_mp = MessageParser::new(cn.clone(), self.db.clone());
                    self.db.store_connection(cn);
                    local_mp.handle_chunk(l_chunk);
                    remote_mp.handle_chunk(r_chunk);
                    ChunkParser::HandshakeDone {
                        local,
                        local_mp,
                        remote,
                        remote_mp,
                    }
                },
            },
            ChunkParser::HandshakeDone {
                local,
                mut local_mp,
                remote,
                mut remote_mp,
            } => {
                if !incoming {
                    ChunkParser::HandshakeDone {
                        local: local.handle_data(payload, &mut local_mp),
                        local_mp,
                        remote,
                        remote_mp,
                    }
                } else {
                    ChunkParser::HandshakeDone {
                        local,
                        local_mp,
                        remote: remote.handle_data(payload, &mut remote_mp),
                        remote_mp,
                    }
                }
            },
        };
        let _ = mem::replace(&mut self.chunk_parser, parser);
    }

    pub fn join(self) {}
}
