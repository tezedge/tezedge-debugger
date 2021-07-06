// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{env, time::Duration};

pub async fn get_p2p(params: &str) -> Result<serde_json::value::Value, serde_json::error::Error> {
    let debugger = env::var("DEBUGGER_URL")
        .unwrap();
    let res = reqwest::get(&format!("{}/v3/messages?{}", debugger, params))
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
}

#[tokio::test]
async fn wait() {
    let mut t = 0u8;

    // timeout * duration = 4 minutes
    let timeout = 24u8;
    let duration = Duration::from_secs(10);

    while t < timeout {
        let response = get_p2p("limit=1000")
            .await.unwrap();
        if response.as_array().unwrap().len() == 1000 {
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
        let response = get_p2p(&format!("limit={}", limit))
            .await.unwrap();
        assert_eq!(response.as_array().unwrap().len(), limit);
    }
}

#[tokio::test]
async fn p2p_cursor() {
    for cursor in 0..8 {
        let response = get_p2p(&format!("cursor={}", cursor))
            .await.unwrap();
        assert_eq!(response[0]["id"].as_i64().unwrap(), cursor);
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
        let response = get_p2p(&format!("cursor=999&limit=1000&types={}", ty))
            .await.unwrap();
        *number = response.as_array().unwrap().len();
    }

    // for all type combination
    for i in 0..(types.len() - 1) {
        let &(ty_i, n_ty_i) = &types[i];
        for j in (i + 1)..types.len() {
            let &(ty_j, n_ty_j) = &types[j];
            let response = get_p2p(&format!("cursor=999&limit=1000&types={},{}", ty_i, ty_j))
                .await.unwrap();
            assert_eq!(response.as_array().unwrap().len(), n_ty_i + n_ty_j);
        }
    }
}
