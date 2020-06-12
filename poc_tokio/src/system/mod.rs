use std::sync::atomic::{AtomicUsize, Ordering};

pub mod orchestrator;
pub mod p2p_parser;
pub mod raw_socket_producer;
pub mod nfqueue_producer;

pub mod prelude {
    pub use super::p2p_parser::spawn_p2p_parser;
    pub use super::raw_socket_producer::raw_socket_producer;
    pub use super::nfqueue_producer::nfqueue_producer;
    pub use super::orchestrator::spawn_packet_orchestrator;
}

pub fn build_raw_socket_system() -> std::io::Result<()> {
    raw_socket_producer::raw_socket_producer()
}

pub fn build_nfqueue_system() -> std::io::Result<()> {
    nfqueue_producer::nfqueue_producer()
}

static CAPTURED_DATA: AtomicUsize = AtomicUsize::new(0);

pub fn update_capture(len: usize) {
    let _ = CAPTURED_DATA.fetch_add(len, Ordering::SeqCst);
}

pub fn get_loaded_data() -> usize {
    CAPTURED_DATA.load(Ordering::SeqCst)
}