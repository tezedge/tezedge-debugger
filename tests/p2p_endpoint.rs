// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{debugger_url, get_rpc_as_json};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node

#[tokio::test]
async fn test_p2p_rpc_limit() {

    // TODO: make an env var
    let base_url = format!("{}/{}", debugger_url(), "v2/p2p");

    let limit: usize = 12;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    assert_eq!(response.as_array().unwrap().len(), limit);

    let limit: usize = 25;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    assert_eq!(response.as_array().unwrap().len(), limit);

    let limit: usize = 5;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    assert_eq!(response.as_array().unwrap().len(), limit);
}

#[tokio::test]
async fn test_p2p_rpc_cursor_id() {
    // TODO: make an env var
    let base_url = format!("{}/{}", debugger_url(), "v2/p2p");

    let cursor_id: usize = 15000;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();
    assert_eq!(response[0]["id"], cursor_id);

    let cursor_id: usize = 1000;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();

    assert_eq!(response.as_array().unwrap().len(), 100);
    assert_eq!(response[0]["id"], cursor_id);
    assert_eq!(response[99]["id"], cursor_id - 99);
}

#[tokio::test]
async fn test_p2p_rpc_types() {
    // TODO: make an env var
    let base_url = format!("{}/{}", debugger_url(), "v2/p2p");

    let message_type = "connection_message";
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", message_type)).await.unwrap();

    let response_array = response.as_array().unwrap();
    assert_eq!(response_array.len(), 100);

    for elem in response_array {
        assert_eq!(elem["type"], message_type);
    }

    let message_type = "metadata";
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", message_type)).await.unwrap();

    let response_array = response.as_array().unwrap();
    assert_eq!(response_array.len(), 100);

    for elem in response_array {
        assert_eq!(elem["type"], message_type);
    }

}