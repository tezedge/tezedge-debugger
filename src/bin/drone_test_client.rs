use tezedge_debugger::{
    utility::stream::MessageStream,
    utility::identity::Identity,
};
use lazy_static::lazy_static;
use structopt::StructOpt;
use tokio::net::TcpStream;
use crypto::nonce::Nonce;
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryMessage};
use crypto::crypto_box::precompute;
use tezedge_debugger::utility::stream::EncryptedMessageWriter;
use tezos_messages::p2p::encoding::peer::{PeerMessageResponse, PeerMessage};
use tezos_messages::p2p::encoding::advertise::AdvertiseMessage;
use std::net::{SocketAddr, IpAddr};

lazy_static! {
    static ref IDENTITY: Identity = Identity {
        peer_id: "idsscFHxXoeJjxQsQBeEveayLyvymA".to_string(),
        public_key: "b41df26473332e7225fdad07045112b5ba6bf295a384785c535cf738575ee245".to_string(),
        secret_key: "dc9640dbd8cf50a5475b6a6d65c96af943380a627cea198906a2a8d4fd37decc".to_string(),
        proof_of_work_stamp: "d0e1945cb693c743e82b3e29750ebbc746c14dbc280c6ee6".to_string(),
    };

    static ref NONCE: Nonce = Nonce::random();
}

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short, long, default_value = "3")]
    pub clients: u32,
    #[structopt(short, long, default_value = "3")]
    pub messages: u32,
}

async fn test_client(messages: u32) {
    let stream = TcpStream::connect("127.0.0.1:13030").await
        .expect("failed to connect to test server");
    let (_, mut writer) = MessageStream::from(stream).split();
    let connection_message = ConnectionMessage::new(
        0,
        &IDENTITY.public_key,
        &IDENTITY.proof_of_work_stamp,
        &NONCE.get_bytes(),
        Default::default(),
    );
    let chunk = BinaryChunk::from_content(&connection_message.as_bytes().unwrap()).unwrap();

    writer.write_message(&chunk).await
        .unwrap();

    let precompouted_key = precompute(
        &IDENTITY.public_key,
        &IDENTITY.secret_key,
    ).unwrap();
    let mut writer = EncryptedMessageWriter::new(writer, precompouted_key, NONCE.clone(), IDENTITY.peer_id.clone());

    for _ in 0..messages {
        let message = PeerMessage::Advertise(AdvertiseMessage::new(&[
            SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0)
        ]));
        let message = PeerMessageResponse::from(message);
        writer.write_message(&message).await
            .unwrap();
    }
}

#[tokio::main]
pub async fn main() -> std::io::Result<()> {
    let opts: Opt = Opt::from_args();
    for _ in 0..opts.clients {
        tokio::spawn(test_client(opts.messages));
    }
    Ok(())
}