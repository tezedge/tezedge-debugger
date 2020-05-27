use riker::actors::*;
use std::collections::HashMap;
use crate::utility::{
    p2p_message::P2PMessage,
    http_message::HttpMessage,
};
use crate::storage::Storage;
use crate::actors::processors::p2p_archiver::P2PArchiver;

#[derive(Clone)]
pub struct Processors {
    p2p_processors: HashMap<String, ActorRef<P2PMessage>>,
    rpc_processors: HashMap<String, ActorRef<HttpMessage>>,
    storage: Storage,
}

impl Actor for Processors {
    type Msg = (); // TODO: Add control messages to spawn new processors

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        match ctx.actor_of_args::<P2PArchiver, _>("p2p_archiver", self.storage.clone()) {
            Ok(actor) => {
                self.p2p_processors.insert(actor.name().to_string(), actor);
            }
            Err(err) => {
                log::error!("Failed to create p2p_archiver: {}", err);
            }
        }
    }

    fn recv(&mut self, _: &Context<Self::Msg>, _: Self::Msg, _: Sender) {}
}

