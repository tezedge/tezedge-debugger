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
    let advertise_count = types.get("advertise")
        .expect("expected two advertise messages");
    assert_eq!(2, *conn_count, "expected two connection messages");
    assert_eq!(2, *advertise_count, "expected two advertise messages");
    // [
    //  Object({
    //      "id": Null,
    //      "incoming": Bool(false),
    //      "payload": Array([Object({"disable_mempool": Bool(false), "private_node": Bool(false), "type": String("metadata_message")})]),
    //      "remote_addr": String("192.168.112.3:53748"),
    //      "source_type": String("local"),
    //      "timestamp": Number(1593081235997286519)}
    // ),
    // Object({"id": Null, "incoming": Bool(true), "payload": Array([Object({"disable_mempool": Bool(false), "private_node": Bool(false), "type": String("metadata_message")})]), "remote_addr": String("192.168.112.3:53748"), "source_type": String("remote"), "timestamp": Number(1593081235996941682)}), Object({"id": Null, "incoming": Bool(false), "payload": Array([Object({"message_nonce": String("0a6cbc90a77a9042457c7fec839faab0c43f687311c4ab42"), "port": Number(0), "proof_of_work_stamp": String("d0e1945cb693c743e82b3e29750ebbc746c14dbc280c6ee6"), "public_key": String("idsscFHxXoeJjxQsQBeEveayLyvymA"), "type": String("connection_message"), "versions": Array([])})]), "remote_addr": String("192.168.112.3:53748"), "source_type": String("local"), "timestamp": Number(1593081235996523233)}), Object({"id": Null, "incoming": Bool(true), "payload": Array([Object({"message_nonce": String("d0910f48326294ad6fc7592833e375ad49c18d15d63dd251"), "port": Number(0), "proof_of_work_stamp": String("74ed18aa2c733e0cbde54e2e7fb9dab28665a3a4d3a9cb08"), "public_key": String("idrj5eYTN6BgrzCT1YQh3mCVuWciVr"), "type": String("connection_message"), "versions": Array([])})]), "remote_addr": String("192.168.112.3:53748"), "source_type": String("remote"), "timestamp": Number(1593081235996182253)})]
}