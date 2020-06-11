pub mod p2p_message;
pub mod tcp_packet;

pub mod prelude {
    pub use super::p2p_message::{P2pMessage, PeerMessage};
    pub use super::tcp_packet::Packet;
}