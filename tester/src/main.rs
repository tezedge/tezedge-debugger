// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::Range;
use structopt::StructOpt;
use tezedge_recorder::crypto::crypto_box::CryptoKey;

const START_TIME: i64 = 1626264000;

#[derive(StructOpt)]
enum Args {
    Log {
        range: u8,
    },
    P2pInitiator {
        this: u16,
        peer: u16,
    },
    P2pResponder {
        this: u16,
        peer: u16,
    },
}

fn main() {
    match Args::from_args() {
        Args::Log { range } => {
            prepare_db_range(if range == 0 { 0..5_000 } else { 5_000..10_000 });
        },
        Args::P2pInitiator { this, peer } => {
            generate_p2p(this, peer, true);
        },
        Args::P2pResponder { this, peer } => {
            generate_p2p(this, peer, false);
        },
    }
}

fn generate_p2p(this: u16, peer: u16, initiator: bool) {
    use std::{
        net::{SocketAddr, TcpListener, TcpStream},
        io::{Read, Write},
    };
    use tezedge_recorder::tezos_messages::p2p::{
        encoding::{
            connection::ConnectionMessage,
            metadata::MetadataMessage,
            ack::AckMessage,
        },
        binary_message::{BinaryRead, BinaryWrite, BinaryChunk},
    };
    use tezedge_recorder::crypto::{
        nonce::{NoncePair, generate_nonces},
        crypto_box::{PrecomputedKey, SecretKey, PublicKey},
    };

    fn id(path: &str, port: u16) -> (ConnectionMessage, SecretKey) {
        use std::fs::File;
        use tezedge_recorder::{
            tezos_messages::p2p::encoding::version::NetworkVersion,
            crypto::nonce::Nonce,
        };
        use tezos_identity::Identity;

        let mut identity_json = String::new();
        File::open(path).unwrap().read_to_string(&mut identity_json).unwrap();
        let identity = Identity::from_json(&identity_json).unwrap();
        let version = NetworkVersion::new("TEZOS_MAINNET".to_string(), 0, 1);
        let connection_message = ConnectionMessage::try_new(
            port,
            &identity.public_key,
            &identity.proof_of_work_stamp,
            Nonce::random(),
            version,
        ).unwrap();

        (connection_message, identity.secret_key)
    }

    fn slow_read(stream: &mut impl Read) -> Vec<u8> {
        let mut size = [0; 2];
        stream.read_exact(&mut size).unwrap();
        let len = (size[0] as usize) * 256 + (size[1] as usize);
        let mut content = [0; 0x10000];
        content[0] = size[0];
        content[1] = size[1];
        stream.read_exact(&mut content[2..(len + 2)]).unwrap();
        println!("read: {}", hex::encode(&content[..(len + 2)]));
        content[..(len + 2)].to_vec()
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], this));
    let listener = TcpListener::bind(addr).unwrap();

    if initiator {
        let addr = SocketAddr::from(([127, 0, 0, 1], peer));
        let mut stream = TcpStream::connect(addr).unwrap();
        println!("connect to: {}", addr);
        let (connection_message, sk) = id("tester/identity_i.json", this);

        let temp = connection_message.as_bytes().unwrap();
        let i_chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&i_chunk).unwrap();
        println!("wrote: {}", hex::encode(&i_chunk));
        let r_chunk = slow_read(&mut stream);

        let connection_message = ConnectionMessage::from_bytes(&r_chunk[2..]).unwrap();
        let pk = PublicKey::from_bytes(connection_message.public_key()).unwrap();

        let key = PrecomputedKey::precompute(&pk, &sk);
        let NoncePair { local , remote } = generate_nonces(&i_chunk, &r_chunk, false).unwrap();

        let msg = MetadataMessage::new(false, false);
        let temp = key.encrypt(&msg.as_bytes().unwrap(), &local).unwrap();
        let chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&chunk).unwrap();
        println!("wrote: {}", hex::encode(&chunk));
        let chunk = slow_read(&mut stream);
        let _msg = key.decrypt(&chunk[2..], &remote).unwrap();
        let local = local.increment();
        let remote = remote.increment();

        let msg = AckMessage::Ack;
        let temp = key.encrypt(&msg.as_bytes().unwrap(), &local).unwrap();
        let chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&chunk).unwrap();
        println!("wrote: {}", hex::encode(&chunk));
        let chunk = slow_read(&mut stream);
        let _msg = key.decrypt(&chunk[2..], &remote).unwrap();
        //let local = local.increment();
        //let remote = remote.increment();
    } else {
        println!("listen at: {}", addr);
        let mut stream = {
            let (stream, peer_addr) = listener.accept().unwrap();
            println!("accept {}", peer_addr);
            stream
        };

        let i_chunk = slow_read(&mut stream);
        let connection_message = ConnectionMessage::from_bytes(&i_chunk[2..]).unwrap();
        let pk = PublicKey::from_bytes(connection_message.public_key()).unwrap();
        let (connection_message, sk) = id("tester/identity_r.json", this);
        let temp = connection_message.as_bytes().unwrap();
        let r_chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&r_chunk).unwrap();
        println!("wrote: {}", hex::encode(&i_chunk));

        let key = PrecomputedKey::precompute(&pk, &sk);
        let NoncePair { local , remote } = generate_nonces(&r_chunk, &i_chunk, true).unwrap();

        let chunk = slow_read(&mut stream);
        let _msg = key.decrypt(&chunk[2..], &remote).unwrap();
        let msg = MetadataMessage::new(false, false);
        let temp = key.encrypt(&msg.as_bytes().unwrap(), &local).unwrap();
        let chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&chunk).unwrap();
        println!("wrote: {}", hex::encode(&chunk));
        let local = local.increment();
        let remote = remote.increment();

        let chunk = slow_read(&mut stream);
        let _msg = key.decrypt(&chunk[2..], &remote).unwrap();
        let msg = AckMessage::Ack;
        let temp = key.encrypt(&msg.as_bytes().unwrap(), &local).unwrap();
        let chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
        stream.write_all(&chunk).unwrap();
        println!("wrote: {}", hex::encode(&chunk));
        //let local = local.increment();
        //let remote = remote.increment();
    }

    let _ = listener;
}

fn prepare_db_range(range: Range<i64>) {
    use std::{time::Duration, thread, net::UdpSocket};
    use chrono::prelude::*;

    let socket = UdpSocket::bind("127.0.0.1:54254").unwrap();

    for i in range {
        let timestamp = START_TIME + i;

        let level = match timestamp % 19 {
            1 | 4 | 5 | 8 => "WARN",
            7 | 10 => "ERROR",
            _ => "INFO",
        };

        let local = Local.timestamp(timestamp, 0).to_rfc3339();
        let fake = "Jul 14 12:00:00.000";
        let msg = format!(
            "<27>1 {} wsvl eb3fdbc716e5 665 eb3fdbc716e5 - {} {} some message",
            local,
            fake,
            level,
        );

        let _ = socket.send_to(msg.as_bytes(), "127.0.0.1:10000").unwrap();
        thread::sleep(Duration::from_micros(400));
    }
}
