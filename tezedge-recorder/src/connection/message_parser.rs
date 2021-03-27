// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use super::{
    chunk_parser::ChunkHandler,
    Database,
    tables::{connection, chunk, message},
};

pub struct MessageParser<Db> {
    builder: Option<message::MessageBuilder>,
    cn: connection::Item,
    db: Arc<Db>,
}

impl<Db> MessageParser<Db>
where
    Db: Database,
{
    pub fn new(cn: connection::Item, db: Arc<Db>) -> Self {
        MessageParser {
            builder: None,
            cn,
            db,
        }
    }
}

impl<Db> ChunkHandler for MessageParser<Db>
where
    Db: Database,
{
    fn handle_chunk(&mut self, chunk: chunk::Item) {
        use std::{
            time::{SystemTime, UNIX_EPOCH},
            convert::TryFrom,
        };
        use self::message::MessageBuilder;

        let sender = &chunk.sender;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        match chunk.counter {
            0 => {
                let message = MessageBuilder::connection_message(chunk.plain.len() as u16)
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn, timestamp);
                self.db.store_message(message);
            },
            1 => {
                let message = MessageBuilder::metadata_message(chunk.plain.len())
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn, timestamp);
                self.db.store_message(message);
            },
            2 => {
                let message = MessageBuilder::acknowledge_message(chunk.plain.len())
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn, timestamp);
                self.db.store_message(message);
            },
            _ => {
                let b = self
                    .builder
                    .take()
                    .unwrap_or_else(|| {
                        let six_bytes = <[u8; 6]>::try_from(&chunk.plain[0..6]).unwrap();
                        MessageBuilder::peer_message(six_bytes, chunk.counter)
                    })
                    .link_chunk(chunk.plain.len());
                self.builder = match b {
                    Ok(builder_full) => {
                        let message = builder_full.build(&sender, &self.cn, timestamp);
                        self.db.store_message(message);
                        None
                    },
                    Err(b) => Some(b),
                }
            },
        }
    }
}
