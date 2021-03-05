pub mod p2p;

mod sorted_intersect;

mod db_message;
pub use self::db_message::DbMessage;

mod store;
pub use self::store::Store;

mod secondary_index;
pub use self::secondary_index::SecondaryIndices;

mod indices;

pub type P2pStore = Store<p2p::Message, p2p::Schema, p2p::Indices>;
