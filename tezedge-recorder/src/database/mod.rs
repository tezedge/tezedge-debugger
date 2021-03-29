// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod rocks;

use std::{path::Path, sync::Arc, error::Error};
use serde::Deserialize;
use super::tables::{connection, chunk, message};

pub trait Database {
    fn store_connection(&self, item: connection::Item);
    fn store_chunk(&self, item: chunk::Item);
    fn store_message(&self, item: message::Item);
}

#[derive(Deserialize)]
pub struct ConnectionsFilter {
    pub cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct ChunksFilter {
    pub limit: Option<u64>,
    pub connection_id: Option<String>,
}

#[derive(Deserialize)]
pub struct MessagesFilter {
    pub cursor: Option<u64>,
}

pub trait DatabaseFetch
where
    Self: DatabaseNew,
{
    fn fetch_connections(
        &self,
        filter: &ConnectionsFilter,
        limit: usize,
    ) -> Result<Vec<(connection::Key, connection::Value)>, Self::Error>;

    fn fetch_chunks(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::Value)>, Self::Error>;

    fn fetch_chunks_truncated(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::ValueTruncated)>, Self::Error>;

    fn fetch_chunk(&self, key: &chunk::Key) -> Result<Option<chunk::Value>, Self::Error>;

    fn fetch_messages(
        &self,
        filter: &MessagesFilter,
        limit: usize,
    ) -> Result<Vec<message::MessageFrontend>, Self::Error>;
}

pub trait DatabaseNew {
    type Error: 'static + Send + Sync + Error;

    fn open<P>(path: P) -> Result<Arc<Self>, Self::Error>
    where
        P: AsRef<Path>;
}
