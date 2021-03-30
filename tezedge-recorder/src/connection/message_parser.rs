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
    error: bool,
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
            error: false,
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
        use std::convert::TryFrom;
        use self::message::MessageBuilder;

        let too_small = match chunk.counter {
            0 => chunk.plain.len() < 82,
            1 => chunk.plain.len() < 2,
            2 => chunk.plain.is_empty(),
            _ => chunk.plain.len() < 6,
        };

        if self.error || too_small {
            if !self.error {
                log::warn!("cannot parse message, connection: {:?}", self.cn);
            }
            self.error = true;
            self.db.store_chunk(chunk);
            return;
        }

        let sender = &chunk.sender;

        let message = match chunk.counter {
            0 => {
                let message = MessageBuilder::connection_message(chunk.plain.len() as u16)
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn);
                Some(message)
            },
            1 => {
                let message = MessageBuilder::metadata_message(chunk.plain.len())
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn);
                Some(message)
            },
            2 => {
                let message = MessageBuilder::acknowledge_message(chunk.plain.len())
                    .link_chunk(chunk.plain.len())
                    .ok()
                    .unwrap()
                    .build(&sender, &self.cn);
                Some(message)
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
                match b {
                    Ok(builder_full) => {
                        self.builder = None;
                        Some(builder_full.build(&sender, &self.cn))
                    },
                    Err(b) => {
                        self.builder = Some(b);
                        None
                    },
                }
            },
        };

        self.db.store_chunk(chunk);
        if let Some(message) = message {
            self.db.store_message(message);
        }
    }
}
