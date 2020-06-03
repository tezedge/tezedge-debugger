// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{get_rpc_as_json, debugger_url, make_rpc_calls, DEFAULT_LIMIT};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node
 
const V2_ENDPOINT: &str = "v2/rpc";

#[ignore]
#[tokio::test]
async fn test_rpc_rpc() {
    // Note: this test should run with no rpc calls yet made to the node
    // use in CI

    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);
    let response = get_rpc_as_json(&format!("{}", base_url)).await.unwrap();

    let response_array = response.as_array().unwrap();

    // no rpc call was yet made
    assert!(response_array.is_empty());

    make_rpc_calls(1).await;

    let response = get_rpc_as_json(&format!("{}", base_url)).await.unwrap();
    let response_array = response.as_array().unwrap();

    // must equal to 2 becouse the outgoing request and the incoming response
    assert_eq!(response_array.len(), 2);

    make_rpc_calls(100).await;

    let response = get_rpc_as_json(&format!("{}?{}", base_url, "limit=300")).await.unwrap();
    let response_array = response.as_array().unwrap();

    assert_eq!(response_array.len(), 202);
}

#[tokio::test]
async fn test_rpc_rpc_limit() {
    // make 50 rpc calls to ensure we have enough data to test the limit argument
    make_rpc_calls(50).await;

    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    let limit: usize = 25;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    assert_eq!(response.as_array().unwrap().len(), limit);

}

#[tokio::test]
async fn test_rpc_rpc_cursor_id() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    // make sure we have at least 200 messages
    make_rpc_calls(100).await;

    let cursor_id: usize = 0;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();
    assert_eq!(response[0]["id"], cursor_id);

    let cursor_id: usize = 100;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "cursor_id", cursor_id)).await.unwrap();

    assert_eq!(response.as_array().unwrap().len(), DEFAULT_LIMIT);

    for i in 0..DEFAULT_LIMIT {
        assert_eq!(response[i]["id"], cursor_id - i);
    }
}

#[tokio::test]
async fn test_rpc_rpc_combination() {
    let base_url = format!("{}/{}", debugger_url(), V2_ENDPOINT);

    // make sure we have at least 200 messages
    make_rpc_calls(100).await;

    let cursor_id: usize = 100;
    let limit: usize = 50;
    let response = get_rpc_as_json(&format!("{}?{}={}&{}={}", base_url, "cursor_id", cursor_id, "limit", limit)).await.unwrap();

    // should be equal
    assert_eq!(response.as_array().unwrap().len(), limit);

    // check the ids, should be decreasing by 1
    for i in 0..limit {
        assert_eq!(response[i]["id"], cursor_id - i);
    }
}

// TODO: arg remote_address (viable in CI, we have to know the specific ip and port of the caller)