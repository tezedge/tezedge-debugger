// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::thread::sleep;
use std::time::Duration;

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
async fn test_p2p_rpc_all_types() {

    let types = vec![
        "tcp",
        "metadata",
        "connection_message",
        "rest_message",
        "disconnect",
        "swap_request",
        "swap_ack",
        "deactivate",
        
    ];

    let nested_types = vec![
        "get_current_head",
        "current_head",
        "get_block_headers",
        "block_header",
        "get_operations",
        "operation",
        "get_protocols",
        "protocol",
        "get_operation_hashes_for_blocks",
        "operation_hashes_for_block",
        "get_operations_for_blocks",
        "operations_for_blocks",
        "bootstrap",
        "get_current_branch",
        "current_branch",
        "advertise",
    ];

    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    for t in types {
        let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", t)).await.unwrap();

        let response_array = response.as_array().unwrap();
        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["type"], t);
        }
    }

    for t in nested_types {
        let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", t)).await.unwrap();

        let empty_vec = Vec::new();
        let response_array = response.as_array().unwrap_or(&empty_vec);

        if response_array.len() == 0 {
            println!("Empty response for type {}, no message found for this type", t);
        }

        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["message"][0]["type"], t);
        }
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

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_remote_requested() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "remote_requested", false)).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert!(elem["remote_requested"] == false);
    }

    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "remote_requested", true)).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert!(response_array.len() <= DEFAULT_LIMIT);

    for elem in response_array {
        assert_eq!(elem["remote_requested"], true);
    }
}


#[ignore]
#[tokio::test]
async fn test_p2p_request_tracking() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    // get 1 response to a remote request (incoming: false, remote_requested: true)
    // be optimistic and give the debugger 10 tries
    let mut response: serde_json::Value = serde_json::json!([]);
    let mut counter: usize = 0;
    while counter < 10 {
        response = get_rpc_as_json(&format!("{}?{}", base_url, "remote_requested=true&incoming=false&limit=1")).await.unwrap();
        counter += 1;
        if response.as_array().unwrap().len() > 0 {
            break
        }
        sleep(Duration::from_secs(1));
    }
    
    let response_array = response.as_array().unwrap();

    // check whether we recieved any response
    assert_eq!(response_array.len(), 1);

    // check if we got the desired message with the provided filter
    assert_eq!(response_array[0]["remote_requested"], true);
    assert_eq!(response_array[0]["incoming"], false);

    // track back the request for the outgoing response
    let cursor_id = response_array[0]["request_id"].clone();

    let response = get_rpc_as_json(&format!("{}?{}={}&{}", base_url, "cursor_id", cursor_id, "limit=1")).await.unwrap();

    // flags should be set as: incoming = true, remote_requested = true
    assert_eq!(response[0]["incoming"], true);
    assert_eq!(response[0]["remote_requested"], true);

    // the id and the request_id should be equal, as it is the request (the response should have the same request_id)
    assert_eq!(response[0]["id"], response_array[0]["request_id"]);
}
