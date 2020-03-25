use failure::Error;
use riker::actors::*;
use std::{collections::HashMap, sync::{Arc, Mutex}, net::Ipv4Addr};
use crate::{
    configuration::Identity,
    actors::peer::{Peer, PeerArgs, PeerMessage},
    storage::MessageStore,
    network::tun_bridge::BridgeWriter,
};
use pnet::packet::{
    Packet as _,
    ipv4::MutableIpv4Packet,
};

#[derive(Debug, PartialEq)]
pub enum Packet {
    Incoming(MutableIpv4Packet<'static>),
    Outgoing(MutableIpv4Packet<'static>),
}

impl Packet {
    pub fn incoming(packet: MutableIpv4Packet<'static>) -> Self { Self::new(packet, true) }

    pub fn outgoing(packet: MutableIpv4Packet<'static>) -> Self { Self::new(packet, false) }

    pub fn raw_msg(&self) -> &[u8] {
        &self.packet().payload()
    }

    pub fn is_empty(&self) -> bool {
        self.raw_msg().len() == 0
    }

    pub fn is_incoming(&self) -> bool {
        match self {
            Packet::Incoming(_) => true,
            _ => false,
        }
    }

    pub fn is_outgoing(&self) -> bool {
        !self.is_incoming()
    }

    pub fn packet(&self) -> &MutableIpv4Packet<'static> {
        match self {
            Packet::Incoming(packet) | Packet::Outgoing(packet) => packet,
        }
    }

    pub fn packet_mut(&mut self) -> &mut MutableIpv4Packet<'static> {
        match self {
            Packet::Incoming(packet) | Packet::Outgoing(packet) => packet,
        }
    }

    pub fn addr(&self) -> Ipv4Addr {
        match self {
            Packet::Incoming(packet) => packet.get_source(),
            Packet::Outgoing(packet) => packet.get_destination(),
        }
    }

    fn new(inner: MutableIpv4Packet<'static>, incoming: bool) -> Self {
        if incoming {
            Packet::Incoming(inner)
        } else {
            Packet::Outgoing(inner)
        }
    }
}

impl Clone for Packet {
    fn clone(&self) -> Self {
        match self {
            Packet::Incoming(_) => {
                let copy = self.packet().packet().to_vec();
                Packet::Incoming(MutableIpv4Packet::owned(copy).unwrap())
            }
            Packet::Outgoing(_) => {
                let copy = self.packet().packet().to_vec();
                Packet::Outgoing(MutableIpv4Packet::owned(copy).unwrap())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum OrchestratorMessage {
    Inner(Packet),
    Outer(Packet),
}

impl OrchestratorMessage {
    pub fn peer_address(&self) -> Ipv4Addr {
        match &self {
            &OrchestratorMessage::Inner(packet) | &OrchestratorMessage::Outer(packet) => packet.addr(),
        }
    }
}

#[derive(Clone)]
pub struct PacketOrchestratorArgs {
    pub local_identity: Identity,
    pub fake_address: String,
    pub local_address: String,
    pub db: MessageStore,
    pub writer: Arc<Mutex<BridgeWriter>>,
}

/// Main packet router and process orchestrator
pub struct PacketOrchestrator {
    remotes: HashMap<Ipv4Addr, ActorRef<PeerMessage>>,
    local_identity: Identity,
    db: MessageStore,
    writer: Arc<Mutex<BridgeWriter>>,
    fake_address: String,
    local_address: String,
}

impl PacketOrchestrator {
    pub fn new(args: PacketOrchestratorArgs) -> Self {
        Self {
            remotes: Default::default(),
            local_identity: args.local_identity,
            db: args.db,
            writer: args.writer,
            local_address: args.local_address,
            fake_address: args.fake_address,
        }
    }

    fn spawn_peer(&self, ctx: &Context<<Self as Actor>::Msg>, addr: Ipv4Addr) -> Result<ActorRef<PeerMessage>, Error> {
        let peer_name = format!("peer-{}", addr).replace(".", "_");
        let act_ref = ctx.actor_of(Props::new_args(Peer::new, PeerArgs {
            addr,
            local_identity: self.local_identity.clone(),
            db: self.db.clone(),
        }), &peer_name)?;
        log::info!("Spawned {}", peer_name);
        Ok(act_ref)
    }
}

impl Actor for PacketOrchestrator {
    type Msg = OrchestratorMessage;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: OrchestratorMessage, _sender: Option<BasicActorRef>) {
        let actor = if let Some(remote) = self.remotes.get_mut(&msg.peer_address()) {
            remote
        } else {
            match self.spawn_peer(ctx, msg.peer_address().clone()) {
                Ok(actor) => {
                    let addr = msg.peer_address().clone();
                    self.remotes.insert(addr.clone(), actor);
                    self.remotes.get_mut(&msg.peer_address()).expect("just inserted actor disappeared")
                }
                Err(e) => {
                    log::warn!("Failed to create actor for messages coming from addr {}: {}", msg.peer_address(), e);
                    return;
                }
            }
        };

        match msg {
            OrchestratorMessage::Inner(packet) => {
                match packet {
                    Packet::Outgoing(_) => {
                        // If the packet is going out, peer will process it and create correct outer packet
                        actor.tell(PeerMessage::Inner(packet), ctx.myself().into());
                    }
                    Packet::Incoming(mut inner) => {
                        // Else the packet came from the internet, and was already processed by a peer
                        let mut bridge = self.writer.lock().expect("mutex poisoning");
                        let _ = bridge.send_packet_to_local(&mut inner, &self.local_address);
                    }
                }
            }
            OrchestratorMessage::Outer(packet) => {
                match packet {
                    Packet::Outgoing(mut inner) => {
                        // Packet processed by the peer, ready to be sent to the internets
                        let mut bridge = self.writer.lock().expect("mutex poisoning");
                        let _ = bridge.send_packet_to_internet(&mut inner, &self.fake_address);
                    }
                    Packet::Incoming(_) => {
                        // Packet needs to be forwarded to correct peer for processing
                        actor.tell(PeerMessage::Outer(packet), ctx.myself().into());
                    }
                }
            }
        }
    }
}