pub mod p2p;

mod sorted_intersect;

mod db_message;

mod store;
pub use self::store::Store;

mod secondary_index;

mod indices;

pub type P2pStore = Store<p2p::Message, p2p::Schema, p2p::Indices>;
