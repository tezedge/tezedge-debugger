// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ws::{CloseCode, Settings, Builder, Sender, Message};
use url;

pub mod common;
use common::{websocket_port, node_url};

#[ignore]
#[tokio::test]
async fn test_rust_ws_connection() {
    // env
    let ws_port = websocket_port();
    let node_url = node_url();

    // construct ws url
    let node_addr = node_url.split(":").skip(1).take(1).last().unwrap();
    let url_string = format!("ws:{}:{}", node_addr, ws_port);

    // build ws 
    let mut ws = Builder::new()
        .with_settings(Settings {
            panic_on_new_connection: true,
            panic_on_protocol: true,
            ..Default::default()
        })
        .build(|output: Sender| {
            move |msg: Message| {
                println!("Got message: {}", msg);
                output.close(CloseCode::Normal)
            }
        })
        .unwrap();

    let url = url::Url::parse(&url_string).unwrap();

    // connect and run
    ws.connect(url).unwrap();
    ws.run().unwrap();
}