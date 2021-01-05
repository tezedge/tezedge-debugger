// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, fmt};
use futures::future::Either;
use tokio::{stream::StreamExt, sync::mpsc};
use serde::{Serialize, Deserialize};
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
use tezos_conversation::{Identity, Conversation, Packet, ConsumeResult, ChunkMetadata, ChunkInfoPair, Sender};
use sniffer::{SocketId, EventId};

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
    pub event_id: EventId,
}

#[derive(Debug)]
pub enum Command {
    GetDebugData,
}

pub struct Parser {
    pub settings: SystemSettings,
    pub source_type: SourceType,
    pub remote_address: SocketAddr,
    pub id: SocketId,
    pub db: mpsc::UnboundedSender<P2pMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserError {
    FailedToWriteInDatabase,
    FailedToDecrypt,
    WrongProofOfWork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionReport {
    pub remote_address: String,
    pub source_type: SourceType,
    pub report: ParserStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserErrorReport {
    pub position: usize,
    pub error: ParserError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserStatistics {
    pub total_chunks: usize,
    pub decrypted_chunks: usize,
    pub error_report: Option<ParserErrorReport>,
}

struct State {
    conversation: Conversation,
    chunk_incoming_counter: usize,
    chunk_outgoing_counter: usize,
    buffer: Vec<u8>,
    statistics: ParserStatistics,
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

    // TODO: split
    pub async fn run<S>(self, mut events: S, mut debug_tx: mpsc::Sender<ConnectionReport>) -> Result<ParserStatistics, ParserStatistics>
    where
        S: Unpin + StreamExt<Item = Either<Message, Command>>,
    {
        let mut state = State {
            conversation: Conversation::new(Self::DEFAULT_POW_TARGET),
            chunk_incoming_counter: 0,
            chunk_outgoing_counter: 0,
            buffer: vec![],
            statistics: ParserStatistics {
                total_chunks: 0,
                decrypted_chunks: 0,
                error_report: None,
            },
        };

        let identity = {
            let identity_json = serde_json::to_string(&self.settings.identity).unwrap();
            Identity::from_json(&identity_json).unwrap()
        };

        // the local socket identifier is pair (pid, fd), but `Conversation` requires the packet
        // have local socket address; it needed only for distinguish between local and remote,
        // let's use fake socket address
        let fake_local = {
            let local_address = self.settings.local_address.clone();
            SocketAddr::new(local_address, 0b1100011110001111)
        };

        while let Some(event) = events.next().await {
            let Message { payload, incoming, counter, event_id } = match event {
                Either::Left(message) => message,
                Either::Right(Command::GetDebugData) => {
                    let report = ConnectionReport {
                        remote_address: self.remote_address.to_string(),
                        source_type: self.source_type.clone(),
                        report: state.statistics.clone(),
                    };
                    debug_tx.send(report).await.unwrap();
                    continue;
                }
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
            let (result, sender, _) = state.conversation.add(Some(&identity), &packet);
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
                    state.inc(incoming, true);
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
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            encrypted.data().to_vec(),
                            decrypted.data().to_vec(),
                            message,
                        );
                        state.inc(incoming, true);
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
                            self.remote_address.clone(),
                            incoming,
                            self.source_type,
                            chunk.data().to_vec(),
                            vec![],
                            Err("cannot decrypt".to_string()),
                        );
                        state.inc(incoming, false);
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
                    state.report_error(ParserError::WrongProofOfWork);
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
                ConsumeResult::UnexpectedChunks | ConsumeResult::InvalidConversation => {
                    state.report_error(ParserError::FailedToDecrypt);
                },
            }
        }

        Ok(state.statistics)
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

    fn store_db(&self, state: &mut State, message: P2pMessage, error_context: DisplayValue<ErrorContext>) -> Result<(), ParserStatistics> {
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

    fn inc(&mut self, incoming: bool, decrypted: bool) {
        self.statistics.total_chunks += 1;
        if decrypted {
            self.statistics.decrypted_chunks += 1;
        }
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
