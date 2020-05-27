use crate::storage::Storage;
use crate::utility::p2p_message::P2PMessage;
use riker::actors::*;
use crate::storage::p2p_store::P2PStore;

pub struct P2PArchiver {
    storage: Storage,
    writers: Vec<ActorRef<(P2PMessage, P2PStore)>>,
    counter: usize,
    writer_count: usize,
}

impl Actor for P2PArchiver {
    type Msg = P2PMessage;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // Spawn writing actors to distribute the load
        for i in 0..self.writer_count {
            let name = format!("{}_p2p_writer_{}", ctx.myself().name(), i);
            match ctx.actor_of::<P2PWriter>(&name) {
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
            writer.send_msg((msg, self.storage.p2p_store()), ctx.myself());
        } else {
            log::error!("No writers present in p2p archiver")
        }
    }
}

impl ActorFactoryArgs<Storage> for P2PArchiver {
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
pub struct P2PWriter;

impl Actor for P2PWriter {
    type Msg = (P2PMessage, P2PStore);

    fn recv(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        let (msg, store) = msg;
        if let Err(err) = store.store_message(&msg) {
            log::error!("Failed to store some p2p message: {}", err);
        }
    }
}

impl ActorFactoryArgs<()> for P2PWriter {
    fn create_args(_: ()) -> Self {
        Self
    }
}