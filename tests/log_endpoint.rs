// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{get_rpc_as_json, debugger_url};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node

const V2_ENDPOINT: &str = "v2/log";

// works only for ocaml node
#[ignore]
#[tokio::test]
async fn test_rpc_log_first() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let response = get_rpc_as_json(&format!("{}?{}", base_url, "cursor_id=0")).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert_eq!(response_array[0]["message"], "Starting the Tezos node...");

}

#[tokio::test]
async fn test_rpc_log_limit() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let limit: usize = 10;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    let response_array = response.as_array().unwrap();
    
    assert!(response_array.len() <= limit);
}

#[tokio::test]
async fn test_rpc_log_cursor_id() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let cursor_id: usize = 10;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();
    let response_array = response.as_array().unwrap();
    
    assert_eq!(response_array.len(), 11);

    for i in 0..11 {
        assert_eq!(response[i]["id"], cursor_id - i);
    }
}

#[tokio::test]
async fn test_rpc_log_combination() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let cursor_id: usize = 100;
    let limit: usize = 10;
    let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "cursor_id", cursor_id, "limit", limit)).await.unwrap();
    let response_array = response.as_array().unwrap();
    
    assert_eq!(response_array.len(), limit);

    for i in 0..limit {
        assert_eq!(response[i]["id"], cursor_id - i);
    }
}

#[tokio::test]
async fn test_rpc_log_level() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let level: &str = "notice";
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "level", level)).await.unwrap();
    let response_array = response.as_array().unwrap();

    for elem in response_array {
        assert_eq!(elem["level"], level);
    }
}