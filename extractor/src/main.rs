use std::{collections::HashMap, path::PathBuf};

use tokio::fs::{File, create_dir};
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() {
    let mapping = [
        ("p2p", "PeerMessageResponse"),
        ("connection", "ConnectionMessage"),
        ("meta", "Metadata"),
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
        fn get_file_name(&self) -> String {
            if self.category == "p2p" {
                format!(
                    "{}.{}.msg",
                    self.id,
                    self.kind.as_ref().expect("kind is expected")
                )
            } else {
                format!("{}.msg", self.id)
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

    let mut cursor = 0;
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
        for message in list {
            if cursor > 0 && message.id > cursor {
                continue
            }

            let url = format!(
                "http://debug.dev.tezedge.com:17742/v3/message/{}",
                message.id
            );
            let decrypted = reqwest::get(url)
                .await
                .unwrap()
                .json::<MessageBytes>()
                .await
                .unwrap();

            let decrypted: Vec<u8> = decrypted.into();
            let path = PathBuf::from(mapping[message.category.as_str()]);
            if !path.exists() {
                create_dir(&path).await.expect("cannot create dir");
            } else {
                assert!(path.is_dir());
            }

            let path = path.join(message.get_file_name());

            println!("-> {}", path.to_string_lossy());
            File::create(path)
                .await
                .expect("cannot create file")
                .write_all(&decrypted)
                .await
                .expect("cannot write data");

            message_id = message.id;
        }
        if message_id == 0 {
            break;
        } else {
            cursor = message_id - 1;
        }
    }
}
