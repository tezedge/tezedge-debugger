use riker::actors::*;
use crate::storage::Storage;
use crate::utility::http_message::{HttpMessage, RPCMessage};
use crate::storage::rpc_store::RPCStore;

pub struct RPCArchiver {
    storage: Storage,
    writers: Vec<ActorRef<(RPCMessage, RPCStore)>>,
    counter: usize,
    writer_count: usize,
}

impl Actor for RPCArchiver {
    type Msg = RPCMessage;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // Spawn writing actors to distribute the load
        for i in 0..self.writer_count {
            let name = format!("{}_rpc_writer_{}", ctx.myself().name(), i);
            match ctx.actor_of::<RPCWriter>(&name) {
                Ok(actor) => self.writers.push(actor),
                Err(err) => log::error!("Failed to create writer {}: {}", name, err),
            }
        }
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        self.counter += 1;
        if self.counter >= self.writers.len() {
            self.counter = 0;
        }

        if let Some(writer) = self.writers.get(self.counter) {
            writer.send_msg((msg, self.storage.rpc_store()), ctx.myself());
        } else {
            log::error!("No writers present in p2p archiver")
        }
    }
}

impl ActorFactoryArgs<Storage> for RPCArchiver {
    fn create_args(storage: Storage) -> Self {
        Self {
            storage,
            writers: Default::default(),
            counter: 0,
            writer_count: 10,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct RPCWriter;

impl Actor for RPCWriter {
    type Msg = (RPCMessage, RPCStore);

    fn recv(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        let (msg, store) = msg;
        if let Err(err) = store.store_message(&msg) {
            log::error!("Failed to store some p2p message: {}", err);
        }
    }
}

impl ActorFactoryArgs<()> for RPCWriter {
    fn create_args(_: ()) -> Self {
        Self
    }
}