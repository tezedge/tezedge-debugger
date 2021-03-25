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
    pub cursor: Option<connection::Key>,
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
    ) -> Result<Vec<connection::Item>, Self::Error>;

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
