// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod common;
use common::{get_rpc_as_json, debugger_url};

/// Running these tests requires a running instance of the tezedge debugger with a tezos node

#[tokio::test]
async fn test_rpc_log_first() {
    let base_url = format!("{}/{}", debugger_url(), "v2/log");

    let response = get_rpc_as_json(&format!("{}?{}", base_url, "cursor_id=0")).await.unwrap();
    let response_array = response.as_array().unwrap();
    assert_eq!(response_array[0]["message"], "Starting the Tezos node...");

}