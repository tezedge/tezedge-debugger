pub mod orchestrator;
pub mod p2p_parser;
pub mod raw_socket_producer;

pub mod prelude {
    pub use super::p2p_parser::spawn_p2p_parser;
    pub use super::raw_socket_producer::raw_socket_producer;
    pub use super::orchestrator::spawn_packet_orchestrator;
}

pub fn build_raw_socket_system() -> std::io::Result<()> {
    raw_socket_producer::raw_socket_producer()
}