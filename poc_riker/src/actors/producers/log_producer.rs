use std::fs::File;
use riker_producer::prelude::*;
use crate::utility::log_message::LogMessage;
use std::io::{BufReader, BufRead};

pub struct LogProducer {
    pub filename: String,
    pub file: Option<File>,
}

impl ProducerBehaviour for LogProducer {
    type Product = Option<LogMessage>;
    type Completed = ();

    fn pre_start(&mut self) -> bool {
        match File::open(&self.filename) {
            Ok(file) => {
                self.file = Some(file);
                true
            }
            Err(err) => {
                log::error!("Failed to open logs file: {}", err);
                false
            }
        }
    }

    fn produce(&mut self) -> ProducerOutput<Self::Product, Self::Completed> {
        if let Some(ref mut file) = self.file {
            let mut reader = BufReader::new(file);
            let mut buf = String::new();
            let read = match reader.read_line(&mut buf) {
                Ok(value) => value,
                Err(err) => {
                    log::error!("Log file closed spontaneously: {}", err);
                    return ProducerOutput::Completed(());
                }
            };
            if read != 0 {
                let line = &buf[..read].trim();
                ProducerOutput::Produced(Some(
                    serde_json::from_str::<LogMessage>(line).ok()
                        .unwrap_or_else(|| LogMessage::raw(line.to_string()))
                ))
            } else {
                ProducerOutput::Produced(None)
            }
        } else {
            log::error!("Log file was never opened");
            ProducerOutput::Completed(())
        }
    }
}

impl ProducerBehaviourFactoryArgs<String> for LogProducer {
    fn create_args(filename: String) -> Self {
        Self {
            filename,
            file: None,
        }
    }
}
