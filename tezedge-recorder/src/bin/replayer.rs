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
    ack::AckMessage, metadata::MetadataMessage, peer::PeerMessageResponse,
};

trait Replayer {
    fn replay_read(&mut self, id: u64) -> Option<()>;
    fn replay_write(&mut self, id: u64);
}

struct State<Rp> {
    replayer: Rp,
    brief: Vec<MessageFrontend>,
    read: bool,
    read_pos: usize,
    write_pos: usize,
}

impl<Rp> Iterator for State<Rp> {
    type Item = (u64, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.read {
            let id = self.brief[self.read_pos..].iter().find_map(|m| {
                if !m.incoming {
                    Some(m.id)
                } else {
                    None
                }
            })?;
            Some((id, true))
        } else {
            let id = self.brief[self.write_pos..].iter().find_map(|m| {
                if m.incoming {
                    Some(m.id)
                } else {
                    None
                }
            })?;
            Some((id, false))
        }
    }
}

impl<Rp> State<Rp> {
    pub fn new(brief: Vec<MessageFrontend>, replayer: Rp) -> Option<Self> {
        if brief.len() < 6 {
            None
        } else {
            Some(State {
                replayer,
                brief,
                read: true,
                read_pos: 6,
                write_pos: 6,
            })
        }
    }
}

impl<Rp> State<Rp>
where
    Rp: Replayer,
{
    pub fn run(self) {
        let mut s = self;
        while let Some((id, read)) = s.next() {
            if read {
                if s.replayer.replay_read(id).is_some() {
                    s.read = s.brief.get(s.write_pos).map(|m| m.incoming).unwrap_or(false);
                } else {
                    s.read = false;
                }
                s.read_pos = (id as usize) + 1;
            } else {
                s.replayer.replay_write(id);
                s.write_pos = (id as usize) + 1;
                s.read = s.brief.get(s.write_pos).map(|m| m.incoming).unwrap_or(false);
            }
        }
        /*for brief in &s.brief[6..] {
            let id = brief.id;
            if brief.incoming {
                s.replayer.replay_write(id);
            } else {
                s.replayer.replay_read(id).unwrap();
            }
        }*/
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

impl Replayer for SimpleReplayer {
    fn replay_read(&mut self, id: u64) -> Option<()> {
        let message = self.db.fetch_message(id).unwrap().unwrap();
        let peer_message = match &message.message {
            &Some(TezosMessage::PeerMessage(ref v)) => v,
            _ => panic!(),
        };

        let r = PeerMessageResponse::read_msg(
            &mut self.stream,
            &mut self.buffer,
            &self.key,
            self.remote.clone(),
            true,
        );
        let (remote, msg) = match r {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                log::info!("pending read {}", id);
                return None;
            },
            Err(e) => {
                panic!("{:?}", e);
            },
        };
        log::info!("replay read {}", id);
        self.remote = remote;

        let _ = (peer_message, msg.message());
        /*let read = serde_json::to_string(&msg.message()).unwrap();
        let have = serde_json::to_string(peer_message).unwrap();
        if read != have {
            log::error!("read: {:?}", msg.message());
            log::error!("have: {:?}", peer_message);
            //panic!();
        }*/

        Some(())
    }

    fn replay_write(&mut self, id: u64) {
        log::info!("replay write {}", id);

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
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let db = Db::open("/volume/debugger_db/tezedge", false).unwrap();

    let mut filter = MessagesFilter::default();
    filter.cursor = Some(0);
    filter.limit = Some(100_000);
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
