use std::{collections::HashMap, path::PathBuf};

use tokio::fs::{File, create_dir};
use tokio::io::AsyncWriteExt;

enum Error {
    ReqwestError(reqwest::Error),
    IOError(std::io::Error),
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::ReqwestError(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}

use std::fmt;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ReqwestError(e) => write!(f, "reqwest error: {}", e),
            Self::IOError(e) => write!(f, "io error: {}", e),
        }
    }
}


#[tokio::main]
async fn main() {
    let mapping = [
        ("p2p", "PeerMessageResponse"),
        ("connection", "ConnectionMessage"),
        ("meta", "Metadata"),
        ("ack", "AckMessage"),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<_, _>>();

    #[derive(serde::Deserialize, Debug)]
    struct Message {
        category: String,
        kind: Option<String>,
        id: u64,
        incoming: bool,
    }

    impl Message {
        fn get_file_name(&self, prefix: &str) -> String {
            if self.category == "p2p" {
                format!(
                    "{}.{}.msg",
                    prefix,
                    self.kind.as_ref().expect("kind is expected")
                )
            } else {
                format!("{}.msg", prefix)
            }
        }
    }

    #[derive(serde::Deserialize, Debug)]
    struct MessageBytes {
        decrypted_bytes: Vec<String>,
    }

    impl From<MessageBytes> for Vec<u8> {
        fn from(source: MessageBytes) -> Vec<u8> {
            source
                .decrypted_bytes
                .into_iter()
                .map(|s| hex::decode(s).expect("can't decode")[0])
                .collect()
        }
    }

    #[derive(serde::Serialize)]
    struct Example<'a> {
        ty: &'a str,
        hex: String,
    }

    let mut cursor = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(0);
    loop {
        let url = if cursor == 0 {
            format!(
                "http://debug.dev.tezedge.com:17742/v3/messages?limit={}",
                100
            )
        } else {
            format!(
                "http://debug.dev.tezedge.com:17742/v3/messages?limit={}&cursor={}",
                100, cursor
            )
        };
        let list = reqwest::get(url)
            .await
            .unwrap()
            .json::<Vec<Message>>()
            .await
            .unwrap();

        let mut message_id = 0;

        let mut handles: Vec<tokio::task::JoinHandle<Result<(), Error>>> = vec![];
        for message in list {
            if cursor > 0 && message.id > cursor {
                continue
            }
            message_id = message.id;

            let path = {
                if let Some(name) = mapping.get(message.category.as_str()) {
                    PathBuf::from(name)
                } else {
                    eprintln!("no mapping for {}", message.category);
                    PathBuf::from(&message.category)
                }
            };
            if !path.exists() {
                create_dir(&path).await.expect("cannot create dir");
            } else {
                assert!(path.is_dir());
            }

            handles.push(tokio::spawn(async move {
                let url = format!(
                    "http://debug.dev.tezedge.com:17742/v3/message/{}",
                    message.id
                );
                let decrypted = reqwest::get(url)
                    .await?
                    .json::<MessageBytes>()
                    .await?;

                let decrypted: Vec<u8> = decrypted.into();
                let hash = sha1::Sha1::from(&decrypted).hexdigest();
                let path = path.join(message.get_file_name(&hash));
                if path.exists() {
                    return Ok(());
                }

                println!("-> {} / {}", message_id, path.to_string_lossy());
                File::create(path)
                    .await?
                    .write_all(&decrypted)
                    .await?;

                Ok(())
            }));
        }

        for handle in handles {
            match handle.await {
                Err(e) => eprintln!("Panic in async block: {}", e),
                Ok(Err(e)) => eprintln!("Error in async block: {}", e),
                _ => (),
            }
        }

        if message_id == 0 {
            break;
        } else {
            cursor = message_id - 1;
        }
    }
}
