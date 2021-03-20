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

#[cfg(test)]
mod tests;

pub type P2pStore = Store<p2p::Indices<local::LocalDb>>;
pub type LogStore = Store<log::Indices<local::LocalDb>>;

#[cfg(unix)]
pub type P2pStoreClient = Store<p2p::Indices<remote::DbClient>>;
#[cfg(unix)]
pub type LogStoreClient = Store<log::Indices<remote::DbClient>>;
