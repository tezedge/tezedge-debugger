// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, fmt};
use tokio::{stream::StreamExt, sync::mpsc};
use tracing::field::DisplayValue;
use tezos_messages::p2p::{
    encoding::{
        connection::ConnectionMessage,
        metadata::MetadataMessage,
        ack::AckMessage,
        peer::PeerMessageResponse,
    },
    binary_message::BinaryMessage,
};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_conversation::{Identity, Conversation, Packet, ConsumeResult, ChunkMetadata, ChunkInfoPair};
use sniffer::EventId;

use crate::{
    system::SystemSettings,
    messages::p2p_message::{
        P2pMessage,
        SourceType,
        TezosPeerMessage,
        PartialPeerMessage,
        HandshakeMessage,
    },
};

pub struct Message {
    pub payload: Vec<u8>,
    pub incoming: bool,
    pub counter: u64,
}

pub struct Parser {
    pub settings: SystemSettings,
    pub source_type: SourceType,
    pub remote_address: SocketAddr,
    pub id: EventId,
    pub db: mpsc::UnboundedSender<P2pMessage>,
}

struct State {
    conversation: Conversation,
    chunk_incoming_counter: usize,
    chunk_outgoing_counter: usize,
    buffer: Vec<u8>,
}

struct ErrorContext {
    is_incoming: bool,
    source_type: SourceType,
    remote_address: SocketAddr,
    id: EventId,
    chunk_counter: usize,
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source {:?}, chunk {} {}, id {}:{}, address {}",
            self.source_type,
            self.chunk_counter,
            if self.is_incoming { "incoming" } else { "outgoing" },
            self.id.pid,
            self.id.fd,
            self.remote_address,
        )
    }
}

impl Parser {
    const DEFAULT_POW_TARGET: f64 = 26.0;

    // TODO: split
    pub async fn run<S>(self, mut events: S)
    where
        S: Unpin + StreamExt<Item = Message>,
    {
        let mut state = State {
            conversation: Conversation::new(Self::DEFAULT_POW_TARGET),
            chunk_incoming_counter: 0,
            chunk_outgoing_counter: 0,
            buffer: vec![],
        };

        let identity = {
            let identity_json = serde_json::to_string(&self.settings.identity).unwrap();
            Identity::from_json(&identity_json).unwrap()
        };

        let fake_local = {
            let local_address = self.settings.local_address.clone();
            let id = self.id.clone();
            let port = 3 << 14 | ((id.pid & 0x7f) << 7) as u16 | (id.fd & 0x7f) as u16;
            SocketAddr::new(local_address, port)
        };

        'outer: while let Some(Message { payload, incoming, counter }) = events.next().await {
            let packet = Packet {
                source: if incoming { self.remote_address.clone() } else { fake_local.clone() },
                destination: if incoming { fake_local.clone() } else { self.remote_address.clone() },
                number: counter,
                payload: payload,
            };
            let (result, _, _) = state.conversation.add(Some(&identity), &packet);
            match result {
                ConsumeResult::Pending => (),
                ConsumeResult::ConnectionMessage(chunk_info) => {
                    let message = ConnectionMessage::from_bytes(&chunk_info.data()[2..])
                        .map(HandshakeMessage::ConnectionMessage)
                        .map(TezosPeerMessage::HandshakeMessage)
                        .map_err(|error| error.to_string());
                    let p2p_msg = P2pMessage::new(
                        self.remote_address.clone(),
                        incoming,
                        self.source_type,
                        chunk_info.data().to_vec(),
                        chunk_info.data().to_vec(),
                        message,
                    );
                    if let Err(err) = self.db.send(p2p_msg) {
                        tracing::error!(
                            context = self.error_context(&state, incoming),
                            error = tracing::field::display(&err),
                            "db channel closed abruptly",
                        );
                        break 'outer;
                    }
                    tracing::info!(
                        context = self.error_context(&state, incoming),
                        "connection message",
                    );
                    state.inc(incoming);
                },
                ConsumeResult::Chunks { regular, failed_to_decrypt } => {
                    for ChunkInfoPair { encrypted, decrypted } in regular {
                        let ec = self.error_context(&state, incoming);
                        let message = state.process(decrypted.data(), ec, incoming);
                        let p2p_msg = P2pMessage::new(
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            encrypted.data().to_vec(),
                            decrypted.data().to_vec(),
                            message,
                        );
                        if let Err(err) = self.db.send(p2p_msg) {
                            tracing::error!(
                                context = self.error_context(&state, incoming),
                                error = tracing::field::display(&err),
                                "db channel closed abruptly",
                            );
                            break 'outer;
                        }
                        state.inc(incoming);
                    }
                    for chunk in &failed_to_decrypt {
                        let p2p_msg = P2pMessage::new(
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            chunk.data().to_vec(),
                            vec![],
                            Err("cannot decrypt".to_string()),
                        );
                        if let Err(err) = self.db.send(p2p_msg) {
                            tracing::error!(
                                context = self.error_context(&state, incoming),
                                error = tracing::field::display(&err),
                                "db channel closed abruptly",
                            );
                            break 'outer;
                        }
                        state.inc(incoming);
                    }
                    if !failed_to_decrypt.is_empty() {
                        tracing::warn!(
                            context = self.error_context(&state, incoming),
                            "cannot decrypt",
                        );
                    }
                },
                ConsumeResult::NoDecipher(_) => {
                    tracing::warn!(
                        context = self.error_context(&state, incoming),
                        "identity wrong",
                    );
                },
                ConsumeResult::PowInvalid => {
                    tracing::warn!(
                        context = self.error_context(&state, incoming),
                        "received connection message with wrong pow, maybe foreign packet",
                    );
                },
                ConsumeResult::UnexpectedChunks | ConsumeResult::InvalidConversation => {
                    tracing::warn!(
                        context = self.error_context(&state, incoming),
                        "probably foreign packet",
                    );
                },
            }
        }
    }

    fn error_context(&self, state: &State, is_incoming: bool) -> DisplayValue<ErrorContext> {
        let ctx = ErrorContext {
            is_incoming,
            source_type: self.source_type,
            remote_address: self.remote_address.clone(),
            id: self.id.clone(),
            chunk_counter: state.chunk(is_incoming),
        };
        tracing::field::display(ctx)
    }
}

impl State {
    fn inc(&mut self, incoming: bool) {
        if incoming {
            self.chunk_incoming_counter += 1;
        } else {
            self.chunk_outgoing_counter += 1;
        }
    }

    fn chunk(&self, incoming: bool) -> usize {
        if incoming {
            self.chunk_incoming_counter
        } else {
            self.chunk_outgoing_counter
        }
    }

    fn process(&mut self, decrypted: &[u8], error_context: DisplayValue<ErrorContext>, incoming: bool) -> Result<TezosPeerMessage, String> {
        let length = decrypted.len();
        if length < 18 {
            tracing::error!(
                context = error_context,
                "the chunk is too small",
            );
        }
        let content = &decrypted[2..(length - 16)];
        match self.chunk(incoming) {
            0 => {
                tracing::warn!(
                    context = error_context,
                    "Connection message should not come here",
                );
                ConnectionMessage::from_bytes(&decrypted[2..])
                    .map(HandshakeMessage::ConnectionMessage)
                    .map(TezosPeerMessage::HandshakeMessage)
                    .map_err(|error| error.to_string())
            },
            1 => MetadataMessage::from_bytes(content)
                    .map(HandshakeMessage::MetadataMessage)
                    .map(TezosPeerMessage::HandshakeMessage)
                    .map_err(|error| error.to_string()),
            2 => AckMessage::from_bytes(content)
                    .map(HandshakeMessage::AckMessage)
                    .map(TezosPeerMessage::HandshakeMessage)
                    .map_err(|error| error.to_string()),
            _ => self.process_peer_message(content),
        }
    }

    fn process_peer_message(&mut self, content: &[u8]) -> Result<TezosPeerMessage, String> {
        self.buffer.extend_from_slice(content);
        match PeerMessageResponse::from_bytes(self.buffer.as_slice()) {
            Err(e) => match &e {
                &BinaryReaderError::Underflow { .. } => {
                    match PartialPeerMessage::from_bytes(self.buffer.as_slice()) {
                        Some(p) => Ok(TezosPeerMessage::PartialPeerMessage(p)),
                        None => Err(e.to_string()),
                    }
                },
                _ => Err(e.to_string()),
            },
            Ok(r) => {
                self.buffer.clear();
                r.messages()
                    .first()
                    .ok_or("empty".to_string())
                    .map(|m| TezosPeerMessage::PeerMessage(m.clone().into()))
            },
        }
    }
}
