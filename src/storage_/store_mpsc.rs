use tokio::sync::mpsc;
use super::store::{StoreCollector, MessageHasId};

pub struct StoreClient<Message>
where
    Message: MessageHasId + Send + Sync + 'static,
{
    tx: mpsc::UnboundedSender<Message>,
}

impl<Message> StoreClient<Message>
where
    Message: MessageHasId + Send + Sync + 'static,
{
    pub fn spawn<StoreServer>(collector: StoreServer) -> Self
    where
        StoreServer: StoreCollector<Message = Message> + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<Message>();
        tokio::spawn(async move {
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                match collector.store_message(msg) {
                    Ok(_) => (),
                    Err(err) => tracing::error!(error = tracing::field::display(&err), "failed to store message"),
                }
            }
        });
        StoreClient { tx }
    }

    pub fn send(&self, message: Message) -> Result<(), ()> {
        self.tx.send(message).map_err(|_| ())
    }
}

impl<Message> Clone for StoreClient<Message>
where
    Message: MessageHasId + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        StoreClient {
            tx: self.tx.clone(),
        }
    }
}
