// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    net::{TcpListener, SocketAddr, TcpStream},
    time::Duration,
    io,
};
use tezedge_recorder::{
    common::MessageCategory,
    database::{DatabaseNew, DatabaseFetch, rocks::Db, MessagesFilter},
    tables::message::{MessageFrontend, TezosMessage},
};
use pseudonode::{ChunkBuffer, Message, handshake};
use crypto::{
    crypto_box::PrecomputedKey,
    nonce::{Nonce, NoncePair},
};
use tezos_messages::p2p::encoding::{
    ack::AckMessage, metadata::MetadataMessage, peer::{PeerMessageResponse, PeerMessage},
    operations_for_blocks::OperationsForBlock,
};

trait Replayer {
    fn replay_read(&mut self, id: u64) -> Option<()>;
    fn replay_write(&mut self, id: u64);
}

struct State<Rp> {
    replayer: Rp,
    brief: Vec<MessageFrontend>,
    read_pos: usize,
    write_pos: usize,
}

impl<Rp> State<Rp> {
    pub fn new(brief: Vec<MessageFrontend>, replayer: Rp) -> Option<Self> {
        if brief.len() < 6 {
            None
        } else {
            Some(State {
                replayer,
                brief,
                read_pos: 6,
                write_pos: 6,
            })
        }
    }

    pub fn next_read(&self) -> Option<u64> {
        self.brief[self.read_pos..].iter().find_map(|m| {
            if !m.incoming {
                Some(m.id)
            } else {
                None
            }
        })
    }

    pub fn next_write(&self) -> Option<u64> {
        self.brief[self.write_pos..].iter().find_map(|m| {
            if m.incoming {
                Some(m.id)
            } else {
                None
            }
        })
    }
}

impl<Rp> State<Rp>
where
    Rp: Replayer,
{
    pub fn run(self) {
        let mut s = self;
        let mut read_timeout = false;
        loop {
            match (s.next_read(), s.next_write()) {
                (None, None) => break,
                (Some(next_read), None) => {
                    s.replayer.replay_read(next_read).unwrap();
                    s.read_pos = (next_read as usize) + 1;
                },
                (None, Some(next_write)) => {
                    s.replayer.replay_write(next_write);
                    s.write_pos = (next_write as usize) + 1;
                },
                (Some(next_read), Some(next_write)) => {
                    if next_write < next_read || read_timeout {
                        s.replayer.replay_write(next_write);
                        s.write_pos = (next_write as usize) + 1;
                        read_timeout = false;
                    } else {
                        match s.replayer.replay_read(next_read) {
                            Some(()) => s.read_pos = (next_read as usize) + 1,
                            None => {
                                log::info!(
                                    "pending read_pos {}, write_pos {}",
                                    next_read,
                                    next_write,
                                );
                                read_timeout = true;
                            }
                        }
                    }
                },
            }
        }
    }
}

pub struct SimpleReplayer {
    db: Db,
    stream: TcpStream,
    buffer: ChunkBuffer,
    key: PrecomputedKey,
    local: Nonce,
    remote: Nonce,
}

impl SimpleReplayer {
    pub fn read(&mut self) -> io::Result<PeerMessage> {
        let r = PeerMessageResponse::read_msg(
            &mut self.stream,
            &mut self.buffer,
            &self.key,
            self.remote.clone(),
            true,
        );

        r.map(|(nonce, msg)| {
            self.remote = nonce;
            msg.message().clone()
        })
    }
}

impl Replayer for SimpleReplayer {
    fn replay_read(&mut self, id: u64) -> Option<()> {
        let have = {
            let message = self.db.fetch_message(id).unwrap().unwrap();
            match message.message {
                Some(TezosMessage::PeerMessage(v)) => v,
                _ => panic!(),
            }
        };

        let read = match self.read() {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                return None;
            },
            Err(e) => {
                Err::<(), _>(e).unwrap();
                unreachable!()
            },
        };

        log::debug!("replay read {}", id);

        if !peer_message_eq(&read, &have) {
            log::error!("mismatch at id: {}, read: {:?}, have: {:?}", id, read, have);
        }

        Some(())
    }

    fn replay_write(&mut self, id: u64) {
        let message = self.db.fetch_message(id).unwrap().unwrap();
        let peer_message = match message.message {
            Some(TezosMessage::PeerMessage(v)) => v,
            _ => panic!(),
        };

        let local = PeerMessageResponse::from(peer_message).write_msg(
            &mut self.stream,
            &self.key,
            self.local.clone(),
        );
        self.local = local;

        log::debug!("replay write {}", id);
    }
}

fn peer_message_eq(lhs: &PeerMessage, rhs: &PeerMessage) -> bool {
    match (&lhs, &rhs) {
        (&PeerMessage::Disconnect, &PeerMessage::Disconnect) => true,
        (&PeerMessage::Advertise(ref lhs), &PeerMessage::Advertise(ref rhs)) => {
            lhs.id() == rhs.id()
        },
        (&PeerMessage::SwapRequest(ref lhs), &PeerMessage::SwapRequest(ref rhs)) => {
            lhs.point() == rhs.point() && lhs.peer_id() == rhs.peer_id()
        },
        (&PeerMessage::SwapAck(ref lhs), &PeerMessage::SwapAck(ref rhs)) => {
            lhs.point() == rhs.point() && lhs.peer_id() == rhs.peer_id()
        },
        (&PeerMessage::Bootstrap, &PeerMessage::Bootstrap) => true,
        (&PeerMessage::GetCurrentBranch(ref lhs), &PeerMessage::GetCurrentBranch(ref rhs)) => {
            lhs.chain_id == rhs.chain_id
        },
        (&PeerMessage::CurrentBranch(ref lhs), &PeerMessage::CurrentBranch(ref rhs)) => {
            lhs.chain_id() == rhs.chain_id() &&
            lhs.current_branch().current_head() == rhs.current_branch().current_head() &&
            lhs.current_branch().history() == rhs.current_branch().history()
        },
        (&PeerMessage::Deactivate(ref lhs), &PeerMessage::Deactivate(ref rhs)) => {
            lhs.deactivate() == rhs.deactivate()
        },
        (&PeerMessage::GetCurrentHead(ref lhs), &PeerMessage::GetCurrentHead(ref rhs)) => {
            lhs.chain_id() == rhs.chain_id()
        },
        (&PeerMessage::CurrentHead(ref lhs), &PeerMessage::CurrentHead(ref rhs)) => {
            lhs.chain_id() == rhs.chain_id() &&
            lhs.current_block_header() == rhs.current_block_header() &&
            lhs.current_mempool() == rhs.current_mempool()
        },
        (&PeerMessage::GetBlockHeaders(ref lhs), &PeerMessage::GetBlockHeaders(ref rhs)) => {
            lhs.get_block_headers() == rhs.get_block_headers()
        },
        (&PeerMessage::BlockHeader(ref lhs), &PeerMessage::BlockHeader(ref rhs)) => {
            lhs.block_header() == rhs.block_header()
        },
        (&PeerMessage::GetOperations(ref lhs), &PeerMessage::GetOperations(ref rhs)) => {
            lhs.get_operations() == rhs.get_operations()
        },
        (&PeerMessage::Operation(ref lhs), &PeerMessage::Operation(ref rhs)) => {
            lhs.operation() == rhs.operation()
        },
        (&PeerMessage::GetProtocols(ref lhs), &PeerMessage::GetProtocols(ref rhs)) => {
            //lhs.get_protocols() == rhs.get_protocols()
            let _ = (lhs, rhs);
            true
        },
        (&PeerMessage::Protocol(ref lhs), &PeerMessage::Protocol(ref rhs)) => {
            //lhs.protocol() == rhs.protocol()
            let _ = (lhs, rhs);
            true
        },
        (&PeerMessage::GetOperationsForBlocks(ref lhs), &PeerMessage::GetOperationsForBlocks(ref rhs)) => {
            let mut lhs: Vec<OperationsForBlock> = lhs.get_operations_for_blocks().clone();
            let mut rhs: Vec<OperationsForBlock> = rhs.get_operations_for_blocks().clone();

            lhs.sort_by(|l, r| l.validation_pass().cmp(&r.validation_pass()));
            rhs.sort_by(|l, r| l.validation_pass().cmp(&r.validation_pass()));
            lhs == rhs
        },
        (&PeerMessage::OperationsForBlocks(ref lhs), &PeerMessage::OperationsForBlocks(ref rhs)) => {
            lhs.operations_for_block() == rhs.operations_for_block() &&
            lhs.operation_hashes_path() == rhs.operation_hashes_path() &&
            lhs.operations() == rhs.operations()
        },
        _ => false,
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let db = Db::open("/volume/debugger_db/tezedge", false, None, None).unwrap();

    let mut filter = MessagesFilter::default();
    filter.cursor = Some(0);
    filter.limit = Some(40_000);
    filter.direction = Some("forward".to_string());
    let brief = db.fetch_messages(&filter).unwrap();

    assert!(matches!(&brief[0].category, &MessageCategory::Connection));
    assert!(!brief[0].incoming);
    assert!(matches!(&brief[1].category, &MessageCategory::Connection));
    assert!(brief[1].incoming);

    assert!(matches!(&brief[2].category, &MessageCategory::Meta));
    assert!(!brief[2].incoming);
    assert!(matches!(&brief[3].category, &MessageCategory::Meta));
    assert!(brief[3].incoming);

    assert!(matches!(&brief[4].category, &MessageCategory::Ack));
    assert!(!brief[4].incoming);
    assert!(matches!(&brief[5].category, &MessageCategory::Ack));
    assert!(brief[5].incoming);

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 9732))).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(1_000)))
        .unwrap();

    let version = match db.fetch_message(0).unwrap().unwrap().message.unwrap() {
        TezosMessage::ConnectionMessage(cm) => Some(cm.version().clone()),
        _ => None,
    };
    let (key, NoncePair { local, remote }) = handshake::responder(
        9732,
        &mut stream,
        include_str!("../../identity_i.json"),
        version.unwrap(),
    );

    let mut buffer = ChunkBuffer::default();
    let (remote, _msg) =
        MetadataMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();
    let local = MetadataMessage::new(false, false).write_msg(&mut stream, &key, local);

    let (remote, _msg) =
        AckMessage::read_msg(&mut stream, &mut buffer, &key, remote, false).unwrap();
    let local = AckMessage::Ack.write_msg(&mut stream, &key, local);

    let replayer = SimpleReplayer {
        db,
        stream,
        buffer: ChunkBuffer::default(),
        key,
        local,
        remote,
    };
    State::new(brief, replayer).map(State::run);
}
