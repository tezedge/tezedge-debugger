use tezedge_debugger::utility::identity::Identity;
use tokio::{
    prelude::*,
    net::{TcpListener, TcpStream},
};
use lazy_static::lazy_static;
use std::net::SocketAddr;

lazy_static! {
    static ref IDENTITY: Identity = Identity {
        peer_id: "idsscFHxXoeJjxQsQBeEveayLyvymA".to_string(),
        public_key: "b41df26473332e7225fdad07045112b5ba6bf295a384785c535cf738575ee245".to_string(),
        secret_key: "dc9640dbd8cf50a5475b6a6d65c96af943380a627cea198906a2a8d4fd37decc".to_string(),
        proof_of_work_stamp: "d0e1945cb693c743e82b3e29750ebbc746c14dbc280c6ee6".to_string(),
    };
}

/// This is server handler, all connection will *ALWAYS* be incoming, from some running drone-client
/// Simple and naive ping server, everything will be sent back without any processing.
/// This way, it should ensure correct Tezos Handshake and correct encodings. Which means, only
/// client should be responsible for correct encryption (of his side, as server will just mirror it).
async fn handle_stream(mut stream: TcpStream, peer_addr: SocketAddr) {
    println!("[{}] Spawned peer handler", peer_addr);
    let buffer_size = stream.recv_buffer_size().unwrap_or(64 * 1024);
    let mut buffer = vec![0u8; buffer_size];
    loop {
        let read = stream.read(&mut buffer).await
            .expect("failed to read from stream");
        if read == 0 {
            println!("[{}] Client finished successfully, closing handler.", peer_addr);
            return;
        }
        let data = &buffer[..read];
        stream.write_all(data).await
            .expect("failed to write to steam");
        println!("[{}] Pinged message back ({} bytes)", peer_addr, data.len());
    }
}

#[tokio::main]
/// Build trivial TCP server with ping handler
pub async fn main() -> std::io::Result<()> {
    let server = "0.0.0.0:13030";
    let mut listener = TcpListener::bind(server).await?;
    println!("Started to listening on \"{}\"", server);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        tokio::spawn(handle_stream(stream, peer_addr));
    }
}