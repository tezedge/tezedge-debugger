// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

use std::{ops::Range, env};
use structopt::StructOpt;

#[derive(StructOpt)]
enum Args {
    Log { range: u8 },
    P2pInitiator { this: u16, peer: u16 },
    P2pResponder { this: u16, peer: u16 },
}

fn main() {
    match Args::from_args() {
        Args::Log { range: 0 } => {
            prepare_db_range(0..5_000);
        },
        Args::Log { range: 1 } => {
            prepare_db_range(5_000..10_000);
        },
        Args::Log { range: 2 } => {
            prepare_db_log_words();
        },
        Args::Log { range: _ } => {
            panic!();
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
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use pseudonode::{handshake, Message, ChunkBuffer};
    use crypto::nonce::NoncePair;
    use tezos_messages::p2p::encoding::{
        metadata::MetadataMessage,
        ack::AckMessage,
        peer::{PeerMessage, PeerMessageResponse},
        version::NetworkVersion,
    };

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], this))).unwrap();
    let version = NetworkVersion::new("TEZOS_MAINNET".to_string(), 0, 1);

    if initiator {
        let addr = SocketAddr::from(([127, 0, 0, 1], peer));
        let mut stream = TcpStream::connect(addr).unwrap();

        let (key, NoncePair { local, remote }) = handshake::initiator(
            this,
            &mut stream,
            include_str!("../../identity_i.json"),
            version,
        );
        let mut buffer = ChunkBuffer::default();

        let local = MetadataMessage::new(false, false).write_msg(&mut stream, &key, local);
        let (remote, _msg) =
            MetadataMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();

        let local = AckMessage::Ack.write_msg(&mut stream, &key, local);
        let (_, _msg) =
            AckMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();

        let fake_operation = serde_json::from_str(include_str!("operation_example.json")).unwrap();
        let local = PeerMessageResponse::from(PeerMessage::Operation(fake_operation)).write_msg(
            &mut stream,
            &key,
            local,
        );
        let _ = local;
    } else {
        let mut stream = {
            let (stream, _) = listener.accept().unwrap();
            stream
        };

        let (key, NoncePair { local, remote }) = handshake::responder(
            this,
            &mut stream,
            include_str!("../../identity_r.json"),
            version,
        );
        let mut buffer = ChunkBuffer::default();
        let (remote, _msg) =
            MetadataMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();
        let local = MetadataMessage::new(false, false).write_msg(&mut stream, &key, local);

        let (remote, _msg) =
            AckMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();
        let _ = AckMessage::Ack.write_msg(&mut stream, &key, local);

        let (remote, msg) =
            PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        assert!(matches!(msg.message(), &PeerMessage::Operation(_)));

        let _ = remote;
    }
    let _ = listener;
}

fn prepare_message(timestamp: i64, level: &str, text: &str) -> String {
    use chrono::prelude::*;

    let local = Local.timestamp(timestamp, 0).to_rfc3339();
    let fake = "Jul 14 12:00:00.000";
    format!(
        "<27>1 {} wsvl eb3fdbc716e5 665 eb3fdbc716e5 - {} {} some {}",
        local, fake, level, text,
    )
}

fn send_stream(msgs: impl Iterator<Item = String>) {
    use std::{time::Duration, thread, net::UdpSocket};

    let socket = UdpSocket::bind("127.0.0.1:54254").unwrap();
    for msg in msgs {
        let _ = socket.send_to(msg.as_bytes(), "127.0.0.1:10000").unwrap();
        thread::sleep(Duration::from_millis(1));
    }
}

fn start_time() -> i64 {
    env::var("START_TIME")
        .map(|s| s.parse::<i64>().unwrap_or(0))
        .unwrap_or(0)
}

fn prepare_db_range(range: Range<i64>) {
    let it = range.map(|i| {
        let timestamp = start_time() + i;

        let level = match timestamp % 19 {
            1 | 4 | 5 | 8 => "WARN",
            7 | 10 => "ERROR",
            _ => "INFO",
        };

        // 16 words
        let text = (0..16).fold(String::new(), |acc, _| {
            // 8 * 2 = 16 symbols each
            format!(
                "{} {}",
                acc,
                hex::encode((0..8).map(|_| rand::random()).collect::<Vec<u8>>())
            )
        });
        prepare_message(timestamp, level, &text)
    });
    send_stream(it);
}

fn prepare_db_log_words() {
    use rand::seq::SliceRandom;

    let words = [
        "peer",
        "branch",
        "head",
        "chain",
        "address",
        "ip",
        "message",
        "connection",
    ];
    let it = (1..256).map(|i| {
        let mut bits = [0, 1, 2, 3, 4, 5, 6, 7];
        bits.shuffle(&mut rand::thread_rng());
        let text = bits.iter().fold(String::new(), |acc, j| {
            if i & (1 << j) != 0 {
                format!("{} {}", acc, words[*j as usize])
            } else {
                acc
            }
        });
        prepare_message(start_time(), "INFO", &text)
    });
    send_stream(it);
}
