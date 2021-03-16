pub mod p2p;
pub mod log;
pub mod indices;

mod sorted_intersect;

mod store;
pub use self::store::{Store, StoreCollector};

mod store_mpsc;
pub use self::store_mpsc::StoreClient;

mod secondary_index;
pub use self::secondary_index::SecondaryIndices;

#[cfg(unix)]
pub mod remote;

pub mod local;

pub type P2pStore = Store<local::LocalDb, p2p::Message, p2p::Schema, p2p::Indices<local::LocalDb>>;
pub type LogStore = Store<local::LocalDb, log::Message, log::Schema, log::Indices<local::LocalDb>>;

#[cfg(unix)]
pub type P2pStoreClient = Store<remote::DbClient, p2p::Message, p2p::Schema, p2p::Indices<remote::DbClient>>;
#[cfg(unix)]
pub type LogStoreClient = Store<remote::DbClient, log::Message, log::Schema, log::Indices<remote::DbClient>>;

#[cfg(test)]
mod tests;
