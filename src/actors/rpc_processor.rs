use failure::Error;
use riker::actors::*;
use crate::actors::peer_message::{RawPacketMessage, SenderMessage};

#[derive(Debug, Clone)]
pub struct RpcArgs {
    pub port: u16,
}

/// Actor for processing RPC calls for controlled node.
pub struct RpcProcessor {
    port: u16,
}

impl RpcProcessor {
    pub fn new(args: RpcArgs) -> Self {
        Self {
            port: args.port,
        }
    }

    fn process_message(&mut self, _msg: &mut RawPacketMessage) -> Result<(), Error> {
        Ok(())
    }
}

impl Actor for RpcProcessor {
    type Msg = RawPacketMessage;

    fn recv(&mut self, ctx: &Context<RawPacketMessage>, mut msg: RawPacketMessage, sender: Sender) {
        let _ = self.process_message(&mut msg);
        if let Some(sender) = sender {
            msg.flip_side();
            if let Err(_) = sender.try_tell(SenderMessage::Process(msg), ctx.myself()) {
                log::error!("unable to reach packet orchestrator with processed packet")
            }
        }
    }
}