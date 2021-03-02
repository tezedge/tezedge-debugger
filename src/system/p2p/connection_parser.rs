// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, net::SocketAddr};
use futures::future::Either;
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
use crypto::{hash::HashType, blake2b};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_conversation::{Identity, Conversation, Packet, ConsumeResult, ChunkMetadata, ChunkInfoPair, Sender};
use sniffer::{SocketId, EventId};

use super::{
    report::{ConnectionReport, ParserError, ParserErrorReport},
    parser::{Message, Command},
    compare::PeerMetadata,
};

use crate::{
    system::NodeConfig,
    messages::p2p_message::{
        P2pMessage,
        SourceType,
        TezosPeerMessage,
        PartialPeerMessage,
        HandshakeMessage,
    },
};

pub struct Parser {
    pub identity: Identity,
    pub config: NodeConfig,
    pub source_type: SourceType,
    pub remote_address: SocketAddr,
    pub id: SocketId,
    pub db: mpsc::UnboundedSender<P2pMessage>,
}

struct State {
    conversation: Conversation,
    chunk_incoming_counter: usize,
    chunk_outgoing_counter: usize,
    buffer: Vec<u8>,
    statistics: ConnectionReport,
    metadata: PeerMetadata,
}

struct ErrorContext {
    is_incoming: bool,
    source_type: SourceType,
    event_id: EventId,
    chunk_counter: usize,
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ source {:?}, chunk {} {}, id {} }}",
            self.source_type,
            self.chunk_counter,
            if self.is_incoming { "incoming" } else { "outgoing" },
            self.event_id,
        )
    }
}

impl Parser {
    const DEFAULT_POW_TARGET: f64 = 26.0;

    pub async fn run<S>(self, events: S, tx_report: mpsc::Sender<ConnectionReport>) -> ConnectionReport
    where
        S: Unpin + StreamExt<Item = Either<Message, Command>>,
    {
        match self.run_inner(events, tx_report).await {
            Ok(report) => report,
            Err(report) => report,
        }
    }

    // TODO: split
    async fn run_inner<S>(self, mut events: S, mut tx_report: mpsc::Sender<ConnectionReport>) -> Result<ConnectionReport, ConnectionReport>
    where
        S: Unpin + StreamExt<Item = Either<Message, Command>>,
    {
        let mut state = State {
            conversation: Conversation::new(Self::DEFAULT_POW_TARGET),
            chunk_incoming_counter: 0,
            chunk_outgoing_counter: 0,
            buffer: vec![],
            statistics: ConnectionReport {
                remote_address: self.remote_address.to_string(),
                source_type: self.source_type.clone(),
                peer_id: None,
                sent_bytes: 0,
                received_bytes: 0,
                incomplete_dropped_messages: 0,
                total_chunks: 0,
                decrypted_chunks: 0,
                error_report: None,
                metadata: None,
            },
            metadata: PeerMetadata::default(),
        };

        // the local socket identifier is pair (pid, fd), but `Conversation` requires the packet
        // have local socket address; it needed only for distinguish between local and remote,
        // let's use fake socket address
        let fake_local = "0.0.0.0:54321".parse::<SocketAddr>().unwrap();

        while let Some(event) = events.next().await {
            let Message { payload, incoming, counter, event_id } = match event {
                Either::Left(message) => message,
                Either::Right(Command::GetReport) => {
                    let report = state.statistics.clone();
                    tx_report.send(report).await.unwrap();
                    continue;
                },
                // TODO:
                Either::Right(Command::Terminate) => break,
            };
            let packet = Packet {
                source: if incoming { self.remote_address.clone() } else { fake_local.clone() },
                destination: if incoming { fake_local.clone() } else { self.remote_address.clone() },
                number: counter,
                payload: payload,
            };
            tracing::debug!(
                context = self.error_context(&state, incoming, &event_id),
                payload = tracing::field::display(hex::encode(packet.payload.as_slice())),
            );
            let (result, sender, _) = state.conversation.add(Some(&self.identity), &packet);
            let ok = match (&sender, &self.source_type) {
                (&Sender::Initiator, &SourceType::Local) => !incoming,
                (&Sender::Initiator, &SourceType::Remote) => incoming,
                (&Sender::Responder, &SourceType::Local) => incoming,
                (&Sender::Responder, &SourceType::Remote) => !incoming,
            };
            if !ok {
                tracing::debug!(
                    context = self.error_context(&state, incoming, &event_id),
                    sender = tracing::field::debug(&sender),
                    payload = tracing::field::display(hex::encode(packet.payload.as_slice())),
                    msg = "the combination is not ok",
                );
            }
            match result {
                ConsumeResult::Pending => (),
                ConsumeResult::ConnectionMessage(chunk_info) => {
                    let message = ConnectionMessage::from_bytes(&chunk_info.data()[2..])
                        .map(|cm: ConnectionMessage| {
                            if incoming {
                                let hash = blake2b::digest_128(&chunk_info.data()[4..36]);
                                state.statistics.peer_id =
                                    Some(HashType::CryptoboxPublicKeyHash.hash_to_b58check(&hash));
                            }
                            HandshakeMessage::ConnectionMessage(cm)
                        })
                        .map(TezosPeerMessage::HandshakeMessage)
                        .map_err(|error| error.to_string());
                    let p2p_msg = P2pMessage::new(
                        self.config.name.clone(),
                        self.remote_address.clone(),
                        incoming,
                        self.source_type,
                        chunk_info.data().to_vec(),
                        chunk_info.data().to_vec(),
                        message,
                    );
                    state.inc(incoming, true, chunk_info.data().len());
                    let error_context = self.error_context(&state, incoming, &event_id);
                    self.store_db(&mut state, p2p_msg, error_context)?;
                    tracing::info!(
                        context = self.error_context(&state, incoming, &event_id),
                        msg = "connection message",
                    );
                },
                ConsumeResult::Chunks { regular, failed_to_decrypt } => {
                    for ChunkInfoPair { encrypted, decrypted } in regular {
                        let ec = self.error_context(&state, incoming, &event_id);
                        let message = state.process(decrypted.data(), ec, incoming);
                        let p2p_msg = P2pMessage::new(
                            self.config.name.clone(),
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            encrypted.data().to_vec(),
                            decrypted.data().to_vec(),
                            message,
                        );
                        state.inc(incoming, true, decrypted.data().len());
                        let error_context = self.error_context(&state, incoming, &event_id);
                        self.store_db(&mut state, p2p_msg, error_context)?;
                    }
                    for chunk in &failed_to_decrypt {
                        let context = self.error_context(&state, incoming, &event_id);
                        if state.statistics.error_report.is_some() {
                            tracing::debug!(context = context, msg = "cannot decrypt");
                        } else {
                            tracing::error!(context = context, msg = "cannot decrypt");
                        }
                        let p2p_msg = P2pMessage::new(
                            self.config.name.clone(),
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            chunk.data().to_vec(),
                            vec![],
                            Err("cannot decrypt".to_string()),
                        );
                        state.inc(incoming, false, chunk.data().len());
                        let error_context = self.error_context(&state, incoming, &event_id);
                        self.store_db(&mut state, p2p_msg, error_context)?;
                    }
                    if !failed_to_decrypt.is_empty() {
                        state.report_error(ParserError::FailedToDecrypt);
                    }
                },
                ConsumeResult::NoDecipher(_) => {
                    let context = self.error_context(&state, incoming, &event_id);
                    if state.statistics.error_report.is_some() {
                        tracing::debug!(context = context, msg = "identity wrong");
                    } else {
                        tracing::error!(context = context, msg = "identity wrong");
                    }
                    state.report_error(ParserError::NoDecipher);
                },
                ConsumeResult::PowInvalid => {
                    let context = self.error_context(&state, incoming, &event_id);
                    let payload = tracing::field::display(hex::encode(packet.payload.as_slice()));
                    if state.statistics.error_report.is_some() {
                        tracing::debug!(context = context, payload = payload, msg = "wrong pow");
                    } else {
                        tracing::error!(context = context, payload = payload, msg = "wrong pow");
                    }
                    state.report_error(ParserError::WrongProofOfWork);
                },
                ConsumeResult::UnexpectedChunks => {
                    state.report_error(ParserError::FirstPacketContainMultipleChunks);
                },
                ConsumeResult::InvalidConversation => {
                    state.report_error(ParserError::Unknown);
                },
            }
        }

        let metadata = state.metadata;
        let mut statistics = state.statistics;
        statistics.metadata = Some(metadata);
        Ok(statistics)
    }

    fn error_context(&self, state: &State, is_incoming: bool, event_id: &EventId) -> DisplayValue<ErrorContext> {
        let ctx = ErrorContext {
            is_incoming,
            source_type: self.source_type,
            event_id: event_id.clone(),
            chunk_counter: state.chunk(is_incoming),
        };
        tracing::field::display(ctx)
    }

    fn store_db(&self, state: &mut State, message: P2pMessage, error_context: DisplayValue<ErrorContext>) -> Result<(), ConnectionReport> {
        self.db.send(message)
            .map_err(|err| {
                tracing::error!(
                    context = error_context,
                    error = tracing::field::display(&err),
                    msg = "db channel closed abruptly",
                );
                state.report_error(ParserError::FailedToWriteInDatabase);
                state.statistics.clone()
            })
    }
}

impl State {
    fn report_error(&mut self, error: ParserError) {
        if self.statistics.error_report.is_none() {
            self.statistics.error_report = Some(ParserErrorReport {
                position: self.statistics.total_chunks,
                error: error,
            })
        }
    }

    fn inc(&mut self, incoming: bool, decrypted: bool, length: usize) {
        self.statistics.total_chunks += 1;
        if decrypted {
            self.statistics.decrypted_chunks += 1;
        }
        if incoming {
            self.statistics.received_bytes += length as u128;
            self.chunk_incoming_counter += 1;
        } else {
            self.statistics.sent_bytes += length as u128;
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
                msg = "the chunk is too small",
            );
        }
        let content = &decrypted[2..(length - 16)];
        match self.chunk(incoming) {
            0 => {
                tracing::warn!(
                    context = error_context,
                    msg = "Connection message should not come here",
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
            _ => self.process_peer_message(content, incoming, error_context),
        }
    }

    fn process_peer_message(&mut self, content: &[u8], incoming: bool, error_context: DisplayValue<ErrorContext>) -> Result<TezosPeerMessage, String> {
        if let Ok(r) = PeerMessageResponse::from_bytes(content) {
            if !self.buffer.is_empty() {
                // previous chunk (or chunks) contains incomplete message,
                // but this chunk is not a continuation, but a new message,
                // should not happen, maybe it is a bug in ocaml node
                tracing::warn!(
                    context = error_context,
                    msg = "incomplete message dropped",
                );
                self.statistics.incomplete_dropped_messages += 1;
                self.buffer.clear();
            }
            return r.messages()
                .first()
                .ok_or("empty".to_string())
                .map(|m| {
                    let m = m.clone().into();
                    self.metadata.count_message(&m, incoming);
                    TezosPeerMessage::PeerMessage(m)
                })
        }

        self.buffer.extend_from_slice(content);
        match PeerMessageResponse::from_bytes(self.buffer.as_slice()) {
            Err(e) => match &e {
                &BinaryReaderError::Underflow { .. } => {
                    match PartialPeerMessage::from_bytes(self.buffer.as_slice()) {
                        Some(p) => Ok(TezosPeerMessage::PartialPeerMessage(p)),
                        None => {
                            self.buffer.clear();
                            Err(e.to_string())
                        },
                    }
                },
                _ => {
                    self.buffer.clear();
                    Err(e.to_string())
                },
            },
            Ok(r) => {
                self.buffer.clear();
                r.messages()
                    .first()
                    .ok_or("empty".to_string())
                    .map(|m| {
                        let m = m.clone().into();
                        self.metadata.count_message(&m, incoming);
                        TezosPeerMessage::PeerMessage(m)
                    })
            },
        }
    }
}
