use tracing::{error};
use tokio::sync::mpsc::{
    UnboundedSender, unbounded_channel,
};
use async_trait::async_trait;
use crate::system::SystemSettings;
use crate::storage::{MessageStore, StoreMessage};

type ProcessorTrait = dyn Processor + Sync + Send + 'static;

#[async_trait]
pub trait Processor {
    async fn process(&mut self, msg: StoreMessage);
}

pub fn spawn_processor(settings: SystemSettings) -> UnboundedSender<StoreMessage> {
    let (sender, mut receiver) = unbounded_channel::<StoreMessage>();

    tokio::spawn(async move {
        let mut processors: Vec<Box<ProcessorTrait>> = Default::default();
        processors.push(Box::new(DatabaseProcessor::new(settings.storage.clone())));
        loop {
            if let Some(message) = receiver.recv().await {
                for processor in processors.iter_mut() {
                    processor.process(message.clone()).await;
                }
            } else {
                error!("p2p processing channel closed unexpectedly");
                break;
            }
        }
    });

    sender
}

struct DatabaseProcessor {
    store: MessageStore,
    sender: UnboundedSender<StoreMessage>,
}

impl DatabaseProcessor {
    pub fn new(store: MessageStore) -> Self {
        let ret = Self {
            sender: Self::start_database_task(store.clone()),
            store,
        };

        ret
    }

    fn start_database_task(store: MessageStore) -> UnboundedSender<StoreMessage> {
        let (sender, mut receiver) = unbounded_channel::<StoreMessage>();
        tokio::spawn(async move {
            loop {
                if let Some(msg) = receiver.recv().await {
                    if let Err(err) = store.p2p().store_message(&msg) {
                        error!(error = display(err), "failed to store message");
                    }
                }
            }
        });
        sender
    }
}

#[async_trait]
impl Processor for DatabaseProcessor {
    async fn process(&mut self, mut msg: StoreMessage) {
        loop {
            if let Err(err) = self.sender.send(msg) {
                error!(error = display(&err), "database channel closed abruptly");
                msg = err.0;
                self.sender = Self::start_database_task(self.store.clone());
            } else {
                return;
            }
        }
    }
}