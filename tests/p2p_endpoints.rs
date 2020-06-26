pub mod common;

use common::{debugger_url, get_rpc_as_json};
use std::collections::HashMap;

const V2_ENDPOINT: &str = "v2/p2p";
const EXPECTED_MESSAGES: usize = 6;

#[tokio::test]
/// Test content of p2p endpoints, if it contains correct count of messages.
async fn tests_p2p_correct_test_output() {
    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    let response = get_rpc_as_json(&base_endpoint)
        .await.unwrap();
    let values = response.as_array()
        .expect("expected array of messages");
    assert_eq!(values.len(), EXPECTED_MESSAGES, "expected four parsed messages");
    let mut types = HashMap::new();
    for value in values {
        use serde_json::Value;
        let value = value.as_object()
            .expect("expected array of objects");

        let values: &[(&'static str, &'static dyn Fn(&Value) -> bool, &str)] = &[
            ("id", &Value::is_number, "number"),
            ("incoming", &Value::is_boolean, "boolean"),
            ("payload", &Value::is_array, "array"),
            ("remote_addr", &Value::is_string, "string"),
            ("source_type", &Value::is_string, "string"),
            ("timestamp", &Value::is_number, "number"),
        ];

        for (field_name, type_check, type_name) in values {
            let field = value.get(*field_name)
                .expect(&format!("{} should be set", field_name));
            assert!(!field.is_null(), "{} must be set", field_name);
            assert!(type_check(field), "{} should be {}", field_name, type_name);
        }

        let payload = value.get("payload").unwrap().as_array().unwrap();
        let value = payload.first().expect("messages should not be empty");
        let message_type = value.get("type")
            .expect("payload type should be set");
        assert!(!message_type.is_null(), "payload type should be set");
        assert!(message_type.is_string(), "payload type should be string");
        let entry = types.entry(message_type.as_str().unwrap().to_string());
        let count = entry.or_insert(0);
        *count += 1;
    }
    let conn_count = types.get("connection_message")
        .expect("expected two connection messages");
    let metadata_count = types.get("metadata_message")
        .expect("expected two metadata messages");
    let advertise_count = types.get("advertise")
        .expect("expected two advertise messages");
    assert_eq!(2, *conn_count, "expected two connection messages");
    assert_eq!(2, *metadata_count, "expected two metadata messages");
    assert_eq!(2, *advertise_count, "expected two advertise messages");
}

#[tokio::test]
async fn test_p2p_limit() {
    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    for limit in 0..=EXPECTED_MESSAGES {
        let response = get_rpc_as_json(&format!("{}?limit={}", base_endpoint, limit))
            .await.unwrap();
        assert_eq!(response.as_array().unwrap().len(), limit);
    }
}

#[ignore]
#[tokio::test]
async fn test_p2p_cursor() {
    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    for cursor_id in 0..=EXPECTED_MESSAGES {
        let response = get_rpc_as_json(&format!("{}?cursor_id={}", base_endpoint, cursor_id))
            .await.unwrap();
        assert_eq!(response[0]["id"], cursor_id, "{}", cursor_id);
    }
}

#[tokio::test]
async fn test_p2p_types() {
    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    let values: &[(&str, usize)] = &[
        ("metadata", 2),
        ("advertise", 0),
        ("connection_message", 2),
    ];
    for (r#type, count) in values {
        let response = get_rpc_as_json(&format!("{}?types={}", base_endpoint, r#type))
            .await.unwrap();
        assert_eq!(response.as_array().unwrap().len(), *count, "{}", r#type);
    }
}

#[tokio::test]
async fn test_p2p_combination_types() {
    let debugger_url = debugger_url();
    let base_endpoint = format!("{}/{}", debugger_url, V2_ENDPOINT);
    let values: &[(&str, usize)] = &[
        ("metadata,connection_message", 4),
        ("metadata,advertise", 2),
        ("connection_message,advertise", 2),
        ("connection_message,metadata", 4),
        ("advertise,connection_message", 2),
        ("advertise,metadata", 2),
        ("advertise,metadata,connection_message", 4),
    ];
    for (r#type, number) in values {
        let response = get_rpc_as_json(&format!("{}?types={}", base_endpoint, *r#type))
            .await.unwrap();
        assert_eq!(response.as_array().unwrap().len(), *number, "{}", r#type);
    }
}