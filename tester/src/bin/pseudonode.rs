// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

use std::ops::Range;
use structopt::StructOpt;

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
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use tester::{handshake, Message, ChunkBuffer};
    //use tester::{RandomState, Generator};
    use crypto::nonce::NoncePair;
    use tezos_messages::p2p::encoding::{
        metadata::MetadataMessage,
        ack::AckMessage,
        //peer::{PeerMessage, PeerMessageResponse},
    };

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], this))).unwrap();

    if initiator {
        let addr = SocketAddr::from(([127, 0, 0, 1], peer));
        let mut stream = TcpStream::connect(addr).unwrap();
    
        let (key, NoncePair { local, remote }) = handshake::initiator(this, &mut stream);
        let mut buffer = ChunkBuffer::default();

        let local = MetadataMessage::new(false, false).write_msg(&mut stream, &key, local);
        let (remote, _msg) =
            MetadataMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();

        let local = AckMessage::Ack.write_msg(&mut stream, &key, local);
        let (_, _msg) = AckMessage::read_msg(&mut stream, &mut buffer, &key, remote, false)
            .unwrap();

        //let mut random_state = RandomState::new(0x1234567788, 10);
        //let local = PeerMessageResponse::from(PeerMessage::GetOperations(random_state.gen())).write_msg(&mut stream, &key, local);
        //let local = PeerMessageResponse::from(PeerMessage::Operation(random_state.gen())).write_msg(&mut stream, &key, local);
        //let local = PeerMessageResponse::from(PeerMessage::GetProtocols(random_state.gen())).write_msg(&mut stream, &key, local);
        //let local = PeerMessageResponse::from(PeerMessage::Protocol(random_state.gen())).write_msg(&mut stream, &key, local);
        //let local = PeerMessageResponse::from(PeerMessage::GetOperationsForBlocks(random_state.gen())).write_msg(&mut stream, &key, local);
        //let local = PeerMessageResponse::from(PeerMessage::OperationsForBlocks(random_state.gen())).write_msg(&mut stream, &key, local);
        let _ = local;
    } else {    
        let mut stream = {
            let (stream, _) = listener.accept().unwrap();
            stream
        };

        let (key, NoncePair { local, remote }) = handshake::responder(this, &mut stream);
        let mut buffer = ChunkBuffer::default();
        let (remote, _msg) =
            MetadataMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();
        let local = MetadataMessage::new(false, false).write_msg(&mut stream, &key, local);

        let (remote, _msg) = AckMessage::read_msg(&mut stream, &mut buffer, &key, remote, false)
            .unwrap();
        let _ = AckMessage::Ack.write_msg(&mut stream, &key, local);

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::GetOperations(_)));

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::Operation(_)));

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::GetProtocols(_)));

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::Protocol(_)));

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::GetOperationsForBlocks(_)));

        //let (remote, msg) = PeerMessageResponse::read_msg(&mut stream, &mut buffer, &key, remote, true).unwrap();
        //assert!(matches!(msg.message(), &PeerMessage::OperationsForBlocks(_)));

        let _ = remote;
    }
    let _ = listener;
}

fn prepare_db_range(range: Range<i64>) {
    use std::{time::Duration, thread, net::UdpSocket};
    use chrono::prelude::*;

    let socket = UdpSocket::bind("127.0.0.1:54254").unwrap();

    for i in range {
        let timestamp = tester::START_TIME + i;

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
        thread::sleep(Duration::from_millis(1));
    }
}
