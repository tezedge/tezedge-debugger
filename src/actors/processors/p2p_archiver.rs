use crate::storage::Storage;
use crate::utility::p2p_message::P2pMessage;
use riker::actors::*;

pub struct P2pArchiver {
    storage: Storage,
}

impl Actor for P2pArchiver {
    type Msg = P2pMessage;

    fn recv(&mut self, _: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        if let Err(err) = self.storage.p2p_store().store_message(&msg) {
            log::error!("Failed to store some p2p message: {}", err);
        }
    }
}

impl ActorFactoryArgs<Storage> for P2pArchiver {
    fn create_args(storage: Storage) -> Self {
        Self {
            storage
        }
    }
}