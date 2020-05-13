use failure::Error;
use crate::actors::logs_message::LogMessage;
use std::fs::{File};
use std::io::{BufReader, BufRead};
use crate::storage::MessageStore;

pub fn make_logs_reader(path: &str, db: MessageStore) -> Result<(), Error> {
    let path = path.to_string();
    std::thread::spawn(move || {
        let mut db = db;
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buf = String::new();
        loop {
            let read = reader.read_line(&mut buf)
                .unwrap();
            if read == 0 {
                continue;
            }
            let line = &buf[..read].trim();
            if let Ok(mut msg) = serde_json::from_str::<LogMessage>(line) {
                let _ = db.log_db().store_message(&mut msg);
            }
            buf.clear()
        }
    });
    Ok(())
}