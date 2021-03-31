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
            _ => {
                if self.builder.is_some() {
                    chunk.plain.is_empty()
                } else {
                    chunk.plain.len() < 6
                }
            },
        };

        if self.error || too_small {
            self.error = true;
            if chunk.bytes.len() >= 2 {
                self.db.store_chunk(chunk);
            }
            return;
        }

        let sender = &chunk.sender;

        let message = match chunk.counter {
            0 => Some(MessageBuilder::connection_message().build(&sender, &self.cn)),
            1 => Some(MessageBuilder::metadata_message().build(&sender, &self.cn)),
            2 => Some(MessageBuilder::acknowledge_message().build(&sender, &self.cn)),
            _ => {
                let building_result = self
                    .builder
                    .take()
                    .unwrap_or_else(|| {
                        let six_bytes = <[u8; 6]>::try_from(&chunk.plain[0..6]).unwrap();
                        MessageBuilder::peer_message(six_bytes, chunk.counter)
                    })
                    .link_chunk(chunk.plain.len());
                match building_result {
                    Ok(builder_full) => Some(builder_full.build(&sender, &self.cn)),
                    Err(builder) => {
                        self.builder = builder;
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

    fn handle_cn(&mut self, cn: connection::Item) {
        self.db.update_connection(cn);
    }
}
