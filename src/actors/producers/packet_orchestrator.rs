use riker::actors::*;
use riker_producer::prelude::*;
use riker::actor::{Sender, Context};
use failure::Error;
use crate::utility::tcp_packet::Packet;
use crate::actors::producers::nfqueue_producer::PacketProducer;
use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr};
use crate::utility::identity::Identity;
use crate::actors::parsers::p2p_parser::{P2PParser, P2PParserArgs};
use crate::actors::parsers::rpc_parser::RPCParser;

type ProducerMsg = ProducerOutput<<PacketProducer as ProducerBehaviour>::Product, <PacketProducer as ProducerBehaviour>::Completed>;

#[derive(Debug, Clone)]
pub struct PacketOrchestratorArgs {
    pub rpc_port: u16,
    pub local_identity: Identity,
    pub local_address: IpAddr,
}

#[derive(Debug, Clone)]
pub struct PacketOrchestrator {
    rpc_port: u16,
    rpc_processor: Option<ActorRef<Packet>>,
    remotes: HashMap<SocketAddr, ActorRef<Packet>>,
    local_identity: Identity,
    local_address: IpAddr,
}

impl PacketOrchestrator {
    fn spawn_peer(&self, ctx: &Context<ProducerMsg>, addr: SocketAddr) -> Result<ActorRef<Packet>, Error> {
        let peer_name = format!("peer-{}", addr).replace(|c: char| {
            c == '.' || c == ':'
        }, "_");
        let act_ref = ctx.actor_of_args::<P2PParser, _>(&peer_name, P2PParserArgs {
            local_identity: self.local_identity.clone(),
            addr: self.local_address.clone(),
        })?;
        log::info!("Spawned {}", peer_name);
        Ok(act_ref)
    }

    fn spawn_rpc(&self, ctx: &Context<ProducerMsg>, port: u16) -> Result<ActorRef<Packet>, Error> {
        let peer_name = format!("rpc-{}", port);
        let act_ref = ctx.actor_of_args::<RPCParser, _>(&peer_name, port)?;
        log::info!("Spawned {}", peer_name);
        Ok(act_ref)
    }
}

impl ProducerProcessor<PacketProducer> for PacketOrchestrator {
    fn post_process(&mut self, ctx: &Context<ProducerOutput<<PacketProducer as ProducerBehaviour>::Product, <PacketProducer as ProducerBehaviour>::Completed>>, value: <PacketProducer as ProducerBehaviour>::Product, _: Sender) -> Option<ProducerControl> {
        if let Some(packet) = value {
            let is_incoming = packet.destination_address().ip() == self.local_address;
            let remote_addr = if is_incoming { packet.source_addr() } else { packet.destination_address() };
            if is_incoming && packet.tcp_packet().dst_port() == self.rpc_port || !is_incoming && packet.tcp_packet().src_port() == self.rpc_port {
                // Process it with HttpParser
                if self.rpc_processor.is_none() {
                    let act = self.spawn_rpc(ctx, self.rpc_port).unwrap();
                    self.rpc_processor = Some(act);
                }
                if let Some(ref mut actor) = self.rpc_processor {
                    actor.send_msg(packet, ctx.myself())
                }
            } else {
                if let Some(remote) = self.remotes.get_mut(&remote_addr) {
                    remote
                } else {
                    match self.spawn_peer(ctx, remote_addr) {
                        Ok(actor) => {
                            self.remotes.insert(remote_addr, actor);
                            self.remotes.get_mut(&remote_addr)
                                .expect("just inserted actor disappeared")
                        }
                        Err(e) => {
                            log::warn!("Failed to create actor for message coming from addr {}: {}", remote_addr, e);
                            return None;
                        }
                    }
                }.send_msg(packet, ctx.myself());
            }
        }
        None
    }
}

impl ProducerProcessorFactoryArgs<PacketProducer, PacketOrchestratorArgs> for PacketOrchestrator {
    fn create_args(args: PacketOrchestratorArgs) -> Self {
        Self {
            rpc_port: args.rpc_port,
            rpc_processor: None,
            remotes: Default::default(),
            local_identity: args.local_identity,
            local_address: args.local_address,
        }
    }
}