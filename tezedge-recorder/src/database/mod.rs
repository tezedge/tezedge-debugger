// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod rocks;

use std::{path::Path, sync::Arc, error::Error};

use super::tables::{connection, chunk, message};

pub trait Database {
    fn store_connection(&self, item: connection::Item);
    fn store_chunk(&self, item: chunk::Item);
    fn store_message(&self, item: message::Item);
}

pub trait DatabaseFetch
where
    Self: DatabaseNew,
{
    fn fetch_connections(
        &self,
        cursor: Option<connection::Key>,
        limit: u64,
    ) -> Result<Vec<connection::Item>, Self::Error>;
}

pub trait DatabaseNew {
    type Error: 'static + Send + Sync + Error;

    fn open<P>(path: P) -> Result<Arc<Self>, Self::Error>
    where
        P: AsRef<Path>;
}
