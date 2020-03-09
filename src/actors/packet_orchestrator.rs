use riker::actors::*;
use failure::Error;
use std::collections::HashMap;
use crate::{
    configuration::Identity,
    network::network_message::NetworkMessage,
    actors::peer::{Peer, Message, PeerArgs},
};

#[derive(Debug, Clone)]
/// Simple packet from raw interface, identified by a port
pub struct Packet {
    pub(crate) port: u16,
    pub(crate) incoming: bool,
    pub(crate) data: Vec<u8>,
}

impl Packet {
    pub fn new(port: u16, incoming: bool, data: Vec<u8>) -> Self {
        Self { port, incoming, data }
    }

    pub fn incoming(port: u16, data: Vec<u8>) -> Self {
        Self::new(port, true, data)
    }

    pub fn outgoing(port: u16, data: Vec<u8>) -> Self {
        Self::new(port, false, data)
    }
}

impl From<Packet> for Message {
    fn from(msg: Packet) -> Self {
        Self::new(if msg.incoming {
            NetworkMessage::incoming(msg.data)
        } else {
            NetworkMessage::outgoing(msg.data)
        })
    }
}

#[derive(Debug, Clone)]
pub struct PacketOrchestratorArgs {
    pub local_identity: Identity,
}

/// Main packet router and process orchestrator
pub struct PacketOrchestrator {
    remotes: HashMap<u16, ActorRef<Message>>,
    local_identity: Identity,
}

impl PacketOrchestrator {
    pub fn new(args: PacketOrchestratorArgs) -> Self {
        Self {
            remotes: Default::default(),
            local_identity: args.local_identity,
        }
    }

    fn spawn_peer(&self, ctx: &Context<<Self as Actor>::Msg>, port: u16) -> Result<ActorRef<Message>, Error> {
        Ok(ctx.actor_of(Props::new_args(Peer::new, PeerArgs { port, local_identity: self.local_identity.clone() }), &format!("peer-{}", port))?)
    }
}

impl Actor for PacketOrchestrator {
    type Msg = Packet;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<BasicActorRef>) {
        if let Some(remote) = self.remotes.get_mut(&msg.port) {
            remote.send_msg(msg.into(), ctx.myself());
        } else {
            match self.spawn_peer(ctx, msg.port) {
                Ok(actor) => {
                    let port = msg.port.clone();
                    actor.send_msg(msg.into(), ctx.myself());
                    self.remotes.insert(port, actor);
                }
                Err(e) => {
                    log::warn!("Failed to create actor for messages coming from port {}: {}", msg.port, e);
                }
            }
        }
    }
}