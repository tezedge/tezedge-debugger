// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::IpAddr, string::ToString};
use tokio::sync::mpsc;
use tracing::{trace, error, warn};
use tezos_messages::p2p::{
    encoding::{
        connection::ConnectionMessage,
        metadata::MetadataMessage,
        ack::AckMessage,
        peer::PeerMessageResponse,
    },
    binary_message::BinaryMessage,
};
use tezos_conversation::{Conversation, Packet, ConsumeResult, Identity, ChunkInfoPair, ChunkMetadata, Sender};
use crate::{
    system::SystemSettings,
    messages::{tcp_packet::Packet as TcpPacket, p2p_message::{P2pMessage, SourceType, TezosPeerMessage}},
};

/// Spawn new p2p parser, returning channel to send packets for processing
pub fn spawn_p2p_parser(
    processor_sender: mpsc::UnboundedSender<P2pMessage>,
    settings: SystemSettings,
) -> mpsc::UnboundedSender<TcpPacket> {
    let (sender, receiver) = mpsc::unbounded_channel::<TcpPacket>();
    tokio::spawn(async move {
        let identity_json = serde_json::to_string(&settings.identity).unwrap();
        let identity = Identity::from_json(&identity_json).unwrap();
        let mut parser = Parser::new(settings.local_address.clone(), receiver, processor_sender, identity);
        while parser.parse_next().await {
            trace!("parsed new message");
        }
    });
    sender
}

/// TcpPacket -> P2pMessage
struct Parser {
    local_ip: IpAddr,
    receiver: mpsc::UnboundedReceiver<TcpPacket>,
    sender: mpsc::UnboundedSender<P2pMessage>,
    identity: Identity,
    conversation: Conversation,
    chunk_counter: usize,
    packet_counter: u64,
}

impl Parser {
    pub const DEFAULT_POW_TARGET: f64 = 26.0;

    pub fn new(
        local_ip: IpAddr,
        receiver: mpsc::UnboundedReceiver<TcpPacket>,
        sender: mpsc::UnboundedSender<P2pMessage>,
        identity: Identity,
    ) -> Self {
        Parser {
            local_ip,
            receiver,
            sender,
            identity,
            conversation: Conversation::new(Self::DEFAULT_POW_TARGET),
            chunk_counter: 0,
            packet_counter: 0,
        }
    }

    pub async fn parse_next(&mut self) -> bool {
        match self.receiver.recv().await {
            Some(packet) => {
                trace!(process_length = packet.ip_buffer().len(), "processing packet");
                let packet = Packet {
                    source: packet.source_address(),
                    destination: packet.destination_address(),
                    number: self.packet_counter,
                    payload: packet.payload().to_vec(),
                };
                self.packet_counter += 1;
                let (result, sender, _) = self.conversation.add(Some(&self.identity), &packet);
                let incoming = packet.source.ip() != self.local_ip;
                let remote_addr = if incoming {
                    assert_eq!(packet.destination.ip(), self.local_ip);
                    packet.source.clone()
                } else {
                    assert_eq!(packet.source.ip(), self.local_ip);
                    packet.destination.clone()
                };
                let source_type = match sender {
                    Sender::Initiator => if incoming { SourceType::Remote } else { SourceType::Local },
                    Sender::Responder => if incoming { SourceType::Local } else { SourceType::Remote },
                };
                match result {
                    ConsumeResult::Pending => true,
                    ConsumeResult::ConnectionMessage(chunk_info) => {
                        let message = ConnectionMessage::from_bytes(&chunk_info.data()[2..])
                            .map(TezosPeerMessage::ConnectionMessage)
                            .map_err(|error| error.to_string());
                        let p2p_msg = P2pMessage::new(
                            remote_addr,
                            incoming,
                            source_type,
                            chunk_info.data().to_vec(),
                            chunk_info.data().to_vec(),
                            message,
                        );
                        if let Err(err) = self.sender.send(p2p_msg) {
                            error!(error = tracing::field::display(&err), "processor channel closed abruptly");
                            return false;
                        }
                        self.chunk_counter += 1;
                        true
                    },
                    ConsumeResult::Chunks { regular, failed_to_decrypt } => {
                        let has_chunks = !regular.is_empty();
                        for ChunkInfoPair { encrypted, decrypted } in regular {
                            let length = decrypted.data().len();
                            if length < 18 {
                                error!("the chunk is too small");
                                return false;
                            }
                            let content = &decrypted.data()[2..(length - 16)];
                            let message = match self.chunk_counter {
                                0 => {
                                    error!("Connection message should not come here");
                                    return false;
                                },
                                1 => MetadataMessage::from_bytes(content)
                                        .map(TezosPeerMessage::MetadataMessage)
                                        .map_err(|error| error.to_string()),
                                2 => AckMessage::from_bytes(content)
                                        .map(TezosPeerMessage::AckMessage)
                                        .map_err(|error| error.to_string()),
                                _ => {
                                    warn!(hex = tracing::field::display(hex::encode(content)), "trying to deserialize");
                                    PeerMessageResponse::from_bytes(content)
                                        .map_err(|error| error.to_string())
                                        .and_then(|r| {
                                            r.messages()
                                                .first()
                                                .ok_or("empty".to_string())
                                                .map(|m| TezosPeerMessage::PeerMessage(m.clone()))
                                        })
                                },
                            };
                            let p2p_msg = P2pMessage::new(
                                remote_addr,
                                incoming,
                                source_type,
                                encrypted.data().to_vec(),
                                decrypted.data().to_vec(),
                                message,
                            );
                            if let Err(err) = self.sender.send(p2p_msg) {
                                error!(error = tracing::field::display(&err), "processor channel closed abruptly");
                                return false;
                            }
                            self.chunk_counter += 1;
                        }
                        if !failed_to_decrypt.is_empty() {
                            warn!("some chunks are failed to decrypt");
                        }
                        for chunk in failed_to_decrypt {
                            let p2p_msg = P2pMessage::new(
                                remote_addr,
                                incoming,
                                source_type,
                                chunk.data().to_vec(),
                                Vec::new(),
                                Err("cannot decrypt".to_string()),
                            );
                            if let Err(err) = self.sender.send(p2p_msg) {
                                error!(error = tracing::field::display(&err), "processor channel closed abruptly");
                                return false;
                            }
                            self.chunk_counter += 1;
                        }
                        has_chunks
                    },
                    ConsumeResult::NoDecipher(_) => {
                        false
                    },
                    ConsumeResult::PowInvalid => {
                        warn!("received connection message with wrong pow, maybe foreign packet");
                        false
                    },
                    ConsumeResult::UnexpectedChunks | ConsumeResult::InvalidConversation => {
                        warn!("probably foreign packet");
                        false
                    },
                }
            }
            None => {
                error!("p2p parser channel closed abruptly");
                false
            }
        }
    }
}
