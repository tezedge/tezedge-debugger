use tokio::sync::mpsc;
use super::store::StoreCollector;

pub trait MessageId {
    fn set_id(&mut self, id: u64);
}

pub struct StoreClient<Message>
where
    Message: MessageId + Send + Sync + 'static,
{
    tx: mpsc::UnboundedSender<Message>,
}

impl<Message> StoreClient<Message>
where
    Message: MessageId + Send + Sync + 'static,
{
    pub fn spawn<StoreServer>(collector: StoreServer, limit: u64) -> Self
    where
        StoreServer: StoreCollector<Message = Message> + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<Message>();
        tokio::spawn(async move {
            let mut rx = rx;
            while let Some(mut msg) = rx.recv().await {
                let index = collector.reserve_index();
                if index >= limit {
                    match collector.delete_message(index - limit) {
                        Ok(_) => (),
                        Err(err) => tracing::error!(error = tracing::field::display(&err), "failed to remove message"),
                    }
                }
                msg.set_id(index);
                match collector.store_message(&msg, index) {
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
    Message: MessageId + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        StoreClient {
            tx: self.tx.clone(),
        }
    }
}
