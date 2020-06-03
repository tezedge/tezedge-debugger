// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{get_rpc_as_json, debugger_url, make_rpc_calls};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node
 
#[ignore]
#[tokio::test]
async fn test_rpc_rpc() {
    // Note: this test should run with no rpc calls yet made to the node
    // maybe rework or 

    let base_url = format!("{}/{}", debugger_url(), "v2/rpc");
    let response = get_rpc_as_json(&format!("{}", base_url)).await.unwrap();

    let response_array = response.as_array().unwrap();

    // no rpc call was yet made
    assert!(response_array.is_empty());

    let _ = get_rpc_as_json(&"http://116.202.128.230:48732/chains/main/blocks/head").await.unwrap();

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

    let base_url = format!("{}/{}", debugger_url(), "v2/rpc");

    let limit: usize = 25;
    let response = get_rpc_as_json(&format!("{}?{}={}", base_url, "limit", limit)).await.unwrap();
    assert_eq!(response.as_array().unwrap().len(), limit);

}