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

        let response_array = response.as_array().unwrap();

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
async fn test_p2p_rpc_source_type_combinations() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    // get implying these are requests
    let known_request_types = vec![
        "get_current_head",
        "get_block_headers",
        "get_operations",
        "get_protocols",
        "get_operation_hashes_for_blocks",
        "get_operations_for_blocks",
        "get_current_branch",
    ];

    // complement types to gets, implying responses
    let known_response_types = vec![
        "current_head",
        "block_header",
        "operation",
        "protocol",
        "operation_hashes_for_block",
        "operations_for_blocks",
        "current_branch",
    ];

    // REQUEST
    // LOCAL -> REMOTE: filter the request types
    let incoming = false;
    for request_type in known_request_types.clone() {
        let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "types", request_type, "incoming", incoming)).await.unwrap();
        let response_array = response.as_array().unwrap();

        // there is a chance that this concrete message type was never sent, this is a legit case, so we do not panic
        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["source_type"], "local");
            assert_eq!(elem["incoming"], incoming);
            assert_eq!(elem["message"][0]["type"], request_type);
        }
    }

    // RESPONSE
    // LOCAL -> REMOTE: filter the response types
    let incoming = false;
    for response_type in known_response_types.clone() {
        let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "types", response_type, "incoming", incoming)).await.unwrap();
        let response_array = response.as_array().unwrap();
        
        // there is a chance that this concrete message type was never sent, this is a legit case, so we do not panic
        assert!(response_array.len() <= DEFAULT_LIMIT);

        println!("Request: {}", format!("{}?{}={}&{}={}", base_url, "types", response_type, "incoming", incoming));

        for elem in response_array {
            assert_eq!(elem["source_type"], "remote");
            assert_eq!(elem["incoming"], incoming);
            assert_eq!(elem["message"][0]["type"], response_type);
        }
    }

    // REQUEST
    // LOCAL <- REMOTE: filter the response types
    let incoming = true;
    for request_type in known_request_types {
        let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "types", request_type, "incoming", incoming)).await.unwrap();
        let response_array = response.as_array().unwrap();
        
        // there is a chance that this concrete message type was never sent, this is a legit case, so we do not panic
        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["source_type"], "remote");
            assert_eq!(elem["incoming"], incoming);
            assert_eq!(elem["message"][0]["type"], request_type);
        }
    }

    // RESPONSE
    // LOCAL <- REMOTE: filter the response types
    let incoming = true;
    for response_type in known_response_types.clone() {
        let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "types", response_type, "incoming", incoming)).await.unwrap();
        let response_array = response.as_array().unwrap();
        
        // there is a chance that this concrete message type was never sent, this is a legit case, so we do not panic
        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["source_type"], "local");
            assert_eq!(elem["incoming"], incoming);
            assert_eq!(elem["message"][0]["type"], response_type);
        }
    }
}

#[ignore]
#[tokio::test]
async fn test_p2p_rpc_incoming_and_no_request_types() {
    // no request-response pattern messages
    // incoming also indicates the source_type
    let types = vec![
        "tcp",
        "metadata",
        "connection_message",
        "rest_message",
        "disconnect",
        "swap_request",
        "swap_ack",
        "deactivate",
        "advertise",
        "bootstrap",
        
    ];

    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    for t in types {
        let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "types", t)).await.unwrap();
        let response_array = response.as_array().unwrap();

        println!("Checking type: {}", t);

        assert!(response_array.len() <= DEFAULT_LIMIT);

        for elem in response_array {
            assert_eq!(elem["type"], t);

            if elem["incoming"].as_bool().unwrap() {
                // the message is incoming, so the source_type is the remote
                assert_eq!(elem["source_type"], "remote");
            } else {
                // the message is NOT incoming (outgoing), so the source_type is the remote
                assert_eq!(elem["source_type"], "local");
            }
        }
    }
}