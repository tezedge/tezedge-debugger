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

pub type P2pStore = Store<p2p::Message, p2p::Schema, p2p::Indices>;
pub type LogStore = Store<log::Message, log::Schema, log::Indices>;

#[cfg(test)]
mod tests;
