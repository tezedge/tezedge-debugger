use crate::storage::Storage;
use riker::actors::*;
use riker_producer::prelude::*;
use crate::actors::producers::log_producer::LogProducer;

pub struct LogArchiver {
    storage: Storage
}

impl ProducerProcessor<LogProducer> for LogArchiver {
    fn post_process(&mut self, _: &Context<ProducerOutput<<LogProducer as ProducerBehaviour>::Product, <LogProducer as ProducerBehaviour>::Completed>>, value: <LogProducer as ProducerBehaviour>::Product, _: Sender) -> Option<ProducerControl> {
        let value = value?;
        if let Err(err) = self.storage.log_store().store_message(&value) {
            log::error!("Failed to store some log message: {}", err);
        }
        None
    }
}

impl ProducerProcessorFactoryArgs<LogProducer, Storage> for LogArchiver {
    fn create_args(storage: Storage) -> Self {
        Self {
            storage
        }
    }
}