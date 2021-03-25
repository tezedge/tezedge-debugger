// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc, convert::TryFrom};
use anyhow::Result;
use thiserror::Error;
use crypto::{CryptoError, proof_of_work, blake2b::Blake2bError, hash::FromBytesError};
use super::{
    key::Key,
    chunk_buffer::Buffer,
    Identity, Database,
    tables::{connection, chunk, message},
};

pub struct Connection<Db> {
    incoming: bool,
    handshake: Handshake,
    input_state: Buffer,
    input_bad_counter: u64,
    input_message_builder: Option<message::MessageBuilder>,
    output_state: Buffer,
    output_bad_counter: u64,
    output_message_builder: Option<message::MessageBuilder>,
    id: u128,
    db: Arc<Db>,
}

enum Handshake {
    Buffering {
        identity: Identity,
        db_item: connection::Item,
    },
    HaveKey {
        connection_id: u128,
        remote_addr: SocketAddr,
        key: Key,
    },
    Error(CryptoError),
}

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

impl<Db> Connection<Db>
where
    Db: Database,
{
    pub fn new(remote_addr: SocketAddr, incoming: bool, identity: Identity, db: Arc<Db>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_item = connection::Item::new(timestamp, incoming, remote_addr);
        Connection {
            incoming,
            handshake: Handshake::Buffering { identity, db_item },
            input_state: Buffer::default(),
            input_bad_counter: 0,
            input_message_builder: None,
            output_state: Buffer::default(),
            output_bad_counter: 0,
            output_message_builder: None,
            id: timestamp,
            db,
        }
    }

    fn buffer_mut(&mut self, incoming: bool) -> &mut Buffer {
        if incoming {
            &mut self.input_state
        } else {
            &mut self.output_state
        }
    }

    pub fn handle_data(&mut self, payload: &[u8], incoming: bool) {
        use std::time::{SystemTime, UNIX_EPOCH};
        use self::message::MessageBuilder;

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

        match &self.handshake {
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
                let counter;
                if incoming {
                    counter = self.input_bad_counter;
                    self.input_bad_counter += 1;
                } else {
                    counter = self.output_bad_counter;
                    self.output_bad_counter += 1;
                };
                let c = chunk::Item::new(self.id, counter, incoming, payload.to_vec(), Vec::new());
                self.db.store_chunk(c);
            },
        };
    }

    pub fn join(self) {}
}
