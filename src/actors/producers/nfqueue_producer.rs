use riker_producer::prelude::*;
use std::sync::{Arc, Mutex};
use nfq::{Queue, Verdict};
use crate::utility::tcp_packet::Packet;

pub struct PacketProducer {
    pub queue: Arc<Mutex<Queue>>,
}

impl ProducerBehaviour for PacketProducer {
    type Product = Option<Packet>;
    type Completed = ();

    fn produce(&mut self) -> ProducerOutput<Self::Product, Self::Completed> {
        let mut queue = self.queue.lock()
            .expect("Mutex poisoining");
        let msg = queue.recv();
        match msg {
            Ok(mut value) => {
                value.set_verdict(Verdict::Accept);
                let ret = ProducerOutput::Produced(Packet::new(value.get_payload()));
                let _ = queue.verdict(value);
                ret
            }
            Err(_) => ProducerOutput::Completed(())
        }
    }
}

impl ProducerBehaviourFactoryArgs<()> for PacketProducer {
    fn create_args(_: ()) -> Self {
        let mut queue = Queue::open().expect("failed to open queue");
        let _ = queue.bind(0);
        Self {
            queue: Arc::new(Mutex::new(queue)),
        }
    }
}