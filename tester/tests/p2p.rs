// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{env, time::Duration};
use tezedge_recorder::tables::message;

pub async fn get_p2p(params: &str, name: &str) -> Result<Vec<message::MessageFrontend>, serde_json::error::Error> {
    let debugger = env::var("DEBUGGER_URL")
        .unwrap();
    let res = reqwest::get(&format!("{}/v2/p2p?node_name={}&{}", debugger, name, params))
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
}

#[tokio::test]
async fn check_messages() {
    use tezedge_recorder::common::MessageCategory;
    //use tezedge_recorder::common::MessageKind;

    let items = get_p2p("limit=100", "initiator").await.unwrap();

    let expected = [
        (0, MessageCategory::Connection, None, false),
        (1, MessageCategory::Connection, None, true),
        (2, MessageCategory::Meta, None, false),
        (3, MessageCategory::Meta, None, true),
        (4, MessageCategory::Ack, None, false),
        (5, MessageCategory::Ack, None, true),
        //(6, MessageCategory::P2p, Some(MessageKind::GetOperations), false),
        //(7, MessageCategory::P2p, Some(MessageKind::Operation), false),
        //(6, MessageCategory::P2p, Some(MessageKind::GetProtocols), false),
        //(7, MessageCategory::P2p, Some(MessageKind::Protocol), false),
        //(8, MessageCategory::P2p, Some(MessageKind::GetOperationsForBlocks), false),
        //(9, MessageCategory::P2p, Some(MessageKind::OperationsForBlocks), false),
    ];

    for (id, category, kind, incoming) in &expected {
        let inc = if *incoming { "incoming" } else { "outgoing" };
        items.iter()
            .find(|msg| {
                msg.id == *id &&
                    msg.category.eq(category) &&
                    msg.kind.eq(kind) &&
                    msg.incoming == *incoming
            })
            .expect(&format!("not found an {} message {:?} {:?}", inc, category, kind));
        println!("found an {} message {:?} {:?}", inc, category, kind);
    }
}

#[tokio::test]
async fn wait() {
    let mut t = 0u8;

    // timeout * duration = 4 minutes
    let timeout = 24u8;
    let duration = Duration::from_secs(10);

    while t < timeout {
        let response = get_p2p("limit=1000", "tezedge")
            .await.unwrap();
        if response.len() == 1000 {
            break;
        } else {
            tokio::time::sleep(duration).await;
            t += 1;
        }
    }
    assert!(t < timeout);
}

#[tokio::test]
async fn p2p_limit() {
    for limit in 0..8 {
        let response = get_p2p(&format!("limit={}", limit), "tezedge")
            .await.unwrap();
        assert_eq!(response.len(), limit);
    }
}

#[tokio::test]
async fn p2p_cursor() {
    for cursor in 0..8 {
        let response = get_p2p(&format!("cursor={}", cursor), "tezedge")
            .await.unwrap();
        assert_eq!(response[0].id, cursor);
    }
}

#[tokio::test]
async fn p2p_types_filter() {
    let mut types = [
        ("connection_message", 0),
        ("metadata", 0),
        ("advertise", 0),
        ("get_block_headers", 0),
        ("block_header", 0),
    ];
    for &mut (ty, ref mut number) in &mut types {
        let response = get_p2p(&format!("cursor=999&limit=1000&types={}", ty), "tezedge")
            .await.unwrap();
        *number = response.len();
    }

    // for all type combination
    for i in 0..(types.len() - 1) {
        let &(ty_i, n_ty_i) = &types[i];
        for j in (i + 1)..types.len() {
            let &(ty_j, n_ty_j) = &types[j];
            let response = get_p2p(&format!("cursor=999&limit=1000&types={},{}", ty_i, ty_j), "tezedge")
                .await.unwrap();
            assert_eq!(response.len(), n_ty_i + n_ty_j);
        }
    }
}
