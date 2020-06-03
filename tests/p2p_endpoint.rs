// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{debugger_url, get_rpc_as_json, DEFAULT_LIMIT};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node

const V2_ENDPOINT: &str = "v2/p2p";

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_limit() {

    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

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

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_cursor_id() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let cursor_id: usize = 15000;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();
    assert_eq!(response[0]["id"], cursor_id);

    let cursor_id: usize = 1000;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();

    assert_eq!(response.as_array().unwrap().len(), DEFAULT_LIMIT);
    for i in 0..DEFAULT_LIMIT {
        assert_eq!(response[i]["id"], cursor_id - i);
    }
}

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_types() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let message_type = "connection_message";
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", message_type)).await.unwrap();

    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert_eq!(elem["type"], message_type);
    }

    let message_type = "metadata";
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", message_type)).await.unwrap();

    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert_eq!(elem["type"], message_type);
    }
}

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_incoming() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "incoming", false)).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert_eq!(elem["incoming"], false);
    }

    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "incoming", true)).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert_eq!(elem["incoming"], true);
    }
}

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_combinations() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let message_type_1 = "connection_message";
    let message_type_2 = "metadata";

    let limit = 10;
    let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "types", message_type_1, "limit", limit)).await.unwrap();
    
    let response_array = response.as_array().unwrap();
    assert_eq!(response_array.len(), limit);

    for elem in response_array {
        assert_eq!(elem["type"], message_type_1);
    }

    let response = get_rpc_as_json(&format!("{}?{}={},{}&{}={}&{}={}", base_url, "types", message_type_1, message_type_2, "limit", limit, "incoming", false)).await.unwrap();
    
    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= limit);
    for elem in response_array {
        assert!(elem["type"] == message_type_1 || elem["type"] == message_type_2);
        assert_eq!(elem["incoming"], false);
    }
}