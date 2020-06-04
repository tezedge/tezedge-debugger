// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use riker::actors::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    net::{IpAddr, SocketAddr},
};
use crate::{
    configuration::Identity,
    actors::{
        peer_message::*,
        peer_processor::{PeerProcessor, PeerArgs},
    },
    storage::MessageStore,
    network::tun_bridge::BridgeWriter,
};
use crate::actors::rpc_processor::{RpcProcessor, RpcArgs};
use riker::system::SystemCmd;

#[derive(Clone)]
/// Arguments required to create PacketOrchestrator actor
pub struct PacketOrchestratorArgs {
    pub rpc_port: u16,
    pub local_identity: Identity,
    pub fake_address: IpAddr,
    pub local_address: IpAddr,
    pub db: MessageStore,
    pub writer: Arc<Mutex<BridgeWriter>>,
}

/// The actor responsible for routing, forwarding and relaying packets between other actors and
/// networking bridge
pub struct PacketOrchestrator {
    rpc_port: u16,
    rpc_processor: Option<ActorRef<RawPacketMessage>>,
    remotes: HashMap<SocketAddr, ActorRef<RawPacketMessage>>,
    local_identity: Identity,
    db: MessageStore,
    writer: Arc<Mutex<BridgeWriter>>,
    fake_address: IpAddr,
    local_address: IpAddr,
}

impl PacketOrchestrator {
    /// Create new PacketOrchestrator actor from given arguments
    pub fn new(args: PacketOrchestratorArgs) -> Self {
        Self {
            rpc_port: args.rpc_port,
            rpc_processor: None,
            remotes: Default::default(),
            local_identity: args.local_identity,
            db: args.db,
            writer: args.writer,
            local_address: args.local_address,
            fake_address: args.fake_address,
        }
    }

    /// Spawn new actor for processing packets from specific remote peer
    fn spawn_peer(&self, ctx: &Context<<Self as Actor>::Msg>, addr: SocketAddr) -> Result<ActorRef<RawPacketMessage>, Error> {
        let peer_name = format!("peer-{}", addr).replace(|c: char| {
            c == '.' || c == ':'
        }, "_");
        let act_ref = ctx.actor_of(Props::new_args(PeerProcessor::new, PeerArgs {
            addr,
            local_identity: self.local_identity.clone(),
            db: self.db.clone(),
        }), &peer_name)?;
        log::info!("Spawned {}", peer_name);
        Ok(act_ref)
    }

    /// Spawn actor for processing node RPC request/responses
    fn spawn_rpc(&self, ctx: &Context<<Self as Actor>::Msg>, port: u16) -> Result<ActorRef<RawPacketMessage>, Error> {
        let peer_name = format!("rpc-{}", port);
        let act_ref = ctx.actor_of(Props::new_args(RpcProcessor::new, RpcArgs {
            port,
            db: self.db.clone(),
        }), &peer_name)?;
        log::info!("Spawned {}", peer_name);
        Ok(act_ref)
    }

    /// Relay packet to other side of tun bridge
    fn relay(&mut self, msg: RawPacketMessage) {
        if msg.is_incoming() {
            let mut bridge = self.writer.lock()
                .expect("Mutex poisoning");
            let _ = bridge.send_packet_to_local(msg, self.local_address);
        } else {
            let mut bridge = self.writer.lock()
                .expect("Mutex poisoning");
            let _ = bridge.send_packet_to_internet(msg, self.fake_address);
        }
    }
}

impl Actor for PacketOrchestrator {
    type Msg = SenderMessage;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        match self.spawn_rpc(ctx, self.rpc_port) {
            Ok(actor) => {
                self.rpc_processor = Some(actor);
            }
            Err(err) => {
                log::error!("Failed to create rpc processing actor for port {}: {}", self.rpc_port, err);
            }
        }
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        match msg {
            SenderMessage::Process(msg) => match msg.character() {
                PacketCharacter::InnerOutgoing | PacketCharacter::OuterIncoming => {
                    // Process rpc packet first
                    if msg.is_incoming() && msg.tcp_packet().dst_port() == 8732 || msg.is_outgoing() && msg.tcp_packet().src_port() == 8732 {
                        if let Some(ref mut remote) = self.rpc_processor {
                            remote.tell(msg, ctx.myself().into())
                        } else {
                            self.relay(msg);
                        }
                        return;
                    }

                    let actor = if let Some(remote) = self.remotes.get_mut(&msg.remote_addr()) {
                        remote
                    } else {
                        match self.spawn_peer(ctx, msg.remote_addr()) {
                            Ok(actor) => {
                                self.remotes.insert(msg.remote_addr(), actor);
                                self.remotes.get_mut(&msg.remote_addr())
                                    .expect("just inserted actor disappeared")
                            }
                            Err(e) => {
                                log::warn!("Failed to create actor for message coming from addr {}: {}", msg.remote_addr(), e);
                                return;
                            }
                        }
                    };

                    if !msg.has_payload() {
                        if msg.is_closing() {
                            log::info!("Peer {} closed connection", actor.name());
                            actor.sys_tell(SystemMsg::Command(SystemCmd::Stop));
                            self.remotes.remove(&msg.remote_addr());
                        }
                        self.relay(msg);
                    } else {
                        actor.send_msg(msg, ctx.myself())
                    }
                }
                _ => self.relay(msg),
            },
            SenderMessage::Relay(msg) => {
                self.relay(msg);
            }
            SenderMessage::Forward(inner, data) => {
                let mut bridge = self.writer.lock()
                    .expect("Mutex poisoning");
                let _ = if inner {
                    bridge.forward_to_internet(&data)
                } else {
                    bridge.forward_to_local(&data)
                };
            }
        }
    }
}