// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod rocks;

use std::{error::Error, net::SocketAddr, path::Path, sync::Arc};
use serde::Deserialize;
use super::{
    tables::{connection, chunk, message},
    common::Initiator,
};

pub trait Database {
    fn store_connection(&self, item: connection::Item);
    fn store_chunk(&self, item: chunk::Item);
    fn store_message(&self, item: message::Item);
}

#[derive(Deserialize)]
pub struct ConnectionsFilter {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct ChunksFilter {
    pub limit: Option<u64>,
    pub cn: Option<String>,
}

#[derive(Deserialize)]
pub struct MessagesFilter {
    pub limit: Option<u64>,
    pub cursor: Option<u64>,
    pub remote_addr: Option<SocketAddr>,
    pub initiator: Option<Initiator>,
    pub sender: Option<bool>,
    //pub types: Vec<P2pType>,
}

pub trait DatabaseFetch
where
    Self: DatabaseNew,
{
    fn fetch_connections(
        &self,
        filter: &ConnectionsFilter,
    ) -> Result<Vec<(connection::Key, connection::Value)>, Self::Error>;

    fn fetch_chunks_truncated(
        &self,
        filter: &ChunksFilter,
    ) -> Result<Vec<(chunk::Key, chunk::ValueTruncated)>, Self::Error>;

    fn fetch_chunk(&self, key: &chunk::Key) -> Result<Option<chunk::Value>, Self::Error>;

    fn fetch_messages(
        &self,
        filter: &MessagesFilter,
    ) -> Result<Vec<message::MessageFrontend>, Self::Error>;
}

pub trait DatabaseNew {
    type Error: 'static + Send + Sync + Error;

    fn open<P>(path: P) -> Result<Arc<Self>, Self::Error>
    where
        P: AsRef<Path>;
}
