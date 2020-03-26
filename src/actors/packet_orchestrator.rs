use failure::Error;
use riker::actors::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    net::IpAddr,
};
use crate::{
    configuration::Identity,
    actors::{
        peer_message::*,
        peer::{Peer, PeerArgs},
    },
    storage::MessageStore,
    network::tun_bridge::BridgeWriter,
};

#[derive(Clone)]
pub struct PacketOrchestratorArgs {
    pub local_identity: Identity,
    pub fake_address: IpAddr,
    pub local_address: IpAddr,
    pub db: MessageStore,
    pub writer: Arc<Mutex<BridgeWriter>>,
}

/// Main packet router and process orchestrator
pub struct PacketOrchestrator {
    remotes: HashMap<IpAddr, ActorRef<RawPacketMessage>>,
    local_identity: Identity,
    db: MessageStore,
    writer: Arc<Mutex<BridgeWriter>>,
    fake_address: IpAddr,
    local_address: IpAddr,
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

    fn spawn_peer(&self, ctx: &Context<<Self as Actor>::Msg>, addr: IpAddr) -> Result<ActorRef<RawPacketMessage>, Error> {
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
    type Msg = RawPacketMessage;

    fn recv(&mut self, ctx: &Context<RawPacketMessage>, msg: RawPacketMessage, _: Sender) {
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

        match msg.character() {
            PacketCharacter::InnerOutgoing | PacketCharacter::OuterIncoming => {
                // InnerOutgoing/OuterIncoming == packet was just received from local/remote node => should be sent for processing.
                actor.tell(msg, ctx.myself().into())
            }
            PacketCharacter::InnerIncoming => {
                // InnerIncoming == packet was already processed and should be forwarded to the local node.
                let mut bridge = self.writer.lock()
                    .expect("Mutex poisoning");
                let _ = bridge.send_packet_to_local(msg, self.local_address);
            }
            PacketCharacter::OuterOutgoing => {
                // OuterOutgoing == packet was already processed and should be forwarded to the remote node.
                let mut bridge = self.writer.lock()
                    .expect("Mutex poisoning");
                let _ = bridge.send_packet_to_internet(msg, self.fake_address);
            }
        }
    }
}