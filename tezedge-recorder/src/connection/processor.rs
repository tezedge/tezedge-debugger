// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc};
use either::Either;
use super::{
    chunk_parser::{Handshake, HandshakeOutput, HandshakeDone},
    Identity, Database,
    common::{Local, Remote, Initiator},
    tables::{connection, message},
};

pub struct Connection<Db> {
    chunk_parser: ChunkParser,
    input_message_builder: Option<message::MessageBuilder>,
    output_message_builder: Option<message::MessageBuilder>,
    db: Arc<Db>,
}

enum ChunkParser {
    Invalid,
    Handshake(Handshake),
    HandshakeDone {
        local: HandshakeDone<Local>,
        remote: HandshakeDone<Remote>,    
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
            input_message_builder: None,
            output_message_builder: None,
            db,
        }
    }

    pub fn handle_data(&mut self, payload: &[u8], incoming: bool) {
        use std::{time::{SystemTime, UNIX_EPOCH}, mem};
        use self::message::MessageBuilder;

        let parser = mem::replace(&mut self.chunk_parser, ChunkParser::Invalid);
        let parser = match parser {
            ChunkParser::Invalid => ChunkParser::Invalid,
            ChunkParser::Handshake(h) => match h.handle_data(payload, incoming) {
                Either::Left(h) => ChunkParser::Handshake(h),
                Either::Right(HandshakeOutput { cn , local, l_chunk, remote, r_chunk }) => {
                    self.db.store_chunk(l_chunk);
                    self.db.store_chunk(r_chunk);
                    self.db.store_connection(cn);
                    ChunkParser::HandshakeDone { local, remote }
                },
            }
            ChunkParser::HandshakeDone { local, remote } => {
                if !incoming {
                    ChunkParser::HandshakeDone {
                        local: local.handle_data(payload, &mut |c| self.db.store_chunk(c)),
                        remote,
                    }
                } else {
                    ChunkParser::HandshakeDone {
                        local,
                        remote: remote.handle_data(payload, &mut |c| self.db.store_chunk(c)),
                    }
                }
            },
        };
        let _ = mem::replace(&mut self.chunk_parser, parser);

        /*match &self.handshake {
            Handshake::Buffering { identity, db_item } => {
                let identity = identity.clone();
                let mut db_item = db_item.clone();
                self.buffer_mut(incoming).handle_data(&payload);

                if self.input_state.have_chunk() && self.output_state.have_chunk() {
                    let (_, incoming_chunk) = self.input_state.next().unwrap();
                    match check(&incoming_chunk) {
                        Ok(peer_id) => db_item.set_peer_id(peer_id),
                        Err(warning) => db_item.add_comment(format!("Incoming: {}", warning)),
                    }
                    let (_, outgoing_chunk) = self.output_state.next().unwrap();
                    match check(&outgoing_chunk) {
                        Ok(peer_id) => debug_assert_eq!(peer_id, identity.peer_id),
                        Err(warning) => db_item.add_comment(format!("Outgoing: {}", warning)),
                    }
                    let (initiator, responder) = if self.incoming {
                        (&incoming_chunk, &outgoing_chunk)
                    } else {
                        (&outgoing_chunk, &incoming_chunk)
                    };

                    match Key::new(&identity, initiator, responder) {
                        Ok(key) => {
                            let ts = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_nanos();

                            let length = (incoming_chunk.len() - 2) as u16;
                            let builder = MessageBuilder::connection_message(length);
                            let message = builder.link_chunk(length as usize).ok().unwrap().build(
                                db_item.id,
                                ts,
                                db_item.remote_addr,
                                self.incoming,
                                true,
                            );
                            self.db.store_message(message);

                            let length = (outgoing_chunk.len() - 2) as u16;
                            let builder = MessageBuilder::connection_message(length);
                            let message = builder.link_chunk(length as usize).ok().unwrap().build(
                                db_item.id,
                                ts,
                                db_item.remote_addr,
                                self.incoming,
                                false,
                            );
                            self.db.store_message(message);

                            self.handshake = Handshake::HaveKey {
                                connection_id: db_item.id,
                                remote_addr: db_item.remote_addr,
                                key,
                            };
                        },
                        Err(error) => {
                            db_item.add_comment(format!("Key calculate error: {}", error));
                            self.handshake = Handshake::Error(error);
                        },
                    }

                    self.db.store_connection(db_item);
                    let c =
                        chunk::Item::new(self.id, 0, true, incoming_chunk.clone(), incoming_chunk);
                    self.db.store_chunk(c);
                    let c =
                        chunk::Item::new(self.id, 0, false, outgoing_chunk.clone(), outgoing_chunk);
                    self.db.store_chunk(c);
                }
            },
            Handshake::HaveKey {
                connection_id,
                remote_addr,
                key,
            } => {
                let mut key = key.clone();
                let connection_id = *connection_id;
                let remote_addr = *remote_addr;
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();

                self.buffer_mut(incoming).handle_data(&payload);

                let db = self.db.clone();
                let id = self.id;
                let mut builder = if incoming {
                    self.input_message_builder.take()
                } else {
                    self.output_message_builder.take()
                };
                let source_remote = self.incoming;

                let it = self.buffer_mut(incoming);
                for (counter, payload) in it {
                    let plain = match key.decrypt(&payload, incoming) {
                        Ok(p) => p,
                        Err(error) => {
                            self.handshake = Handshake::Error(error);
                            return;
                        },
                    };

                    match counter {
                        0 => log::warn!("connection message should not be here"),
                        1 => {
                            let message = MessageBuilder::metadata_message(plain.len())
                                .link_chunk(plain.len())
                                .ok()
                                .unwrap()
                                .build(connection_id, ts, remote_addr, source_remote, incoming);
                            db.store_message(message);
                        },
                        2 => {
                            let message = MessageBuilder::acknowledge_message(plain.len())
                                .link_chunk(plain.len())
                                .ok()
                                .unwrap()
                                .build(connection_id, ts, remote_addr, source_remote, incoming);
                            db.store_message(message);
                        },
                        chunk_number => {
                            let six_bytes = <[u8; 6]>::try_from(&plain[0..6]).unwrap();
                            let b = builder
                                .unwrap_or_else(|| {
                                    MessageBuilder::peer_message(six_bytes, chunk_number)
                                })
                                .link_chunk(plain.len());
                            builder = match b {
                                Ok(builder_full) => {
                                    let message = builder_full.build(
                                        connection_id,
                                        ts,
                                        remote_addr,
                                        source_remote,
                                        incoming,
                                    );
                                    db.store_message(message);
                                    None
                                },
                                Err(b) => Some(b),
                            }
                        },
                    }

                    let c = chunk::Item::new(id, counter, incoming, payload, plain);
                    db.store_chunk(c);
                }
                if incoming {
                    self.input_message_builder = builder;
                } else {
                    self.output_message_builder = builder;
                }
                self.handshake = Handshake::HaveKey {
                    connection_id,
                    remote_addr,
                    key,
                };
            },
            Handshake::Error(_error) => {
                let (counter, p) = self.input_state.cleanup();
                if !p.is_empty() {
                    self.input_bad_counter = counter;
                    let c = chunk::Item::new(self.id, counter, true, p, Vec::new());
                    self.db.store_chunk(c);
                }
                let (counter, p) = self.output_state.cleanup();
                if !p.is_empty() {
                    self.output_bad_counter = counter;
                    let c = chunk::Item::new(self.id, counter, false, p, Vec::new());
                    self.db.store_chunk(c);
                }

                let counter = self.inc_bad_counter(incoming);
                let c = chunk::Item::new(self.id, counter, incoming, payload.to_vec(), Vec::new());
                self.db.store_chunk(c);
            },
        };*/
    }

    pub fn join(self) {}
}
