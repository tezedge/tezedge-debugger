use std::env;

pub const DEFAULT_LIMIT: usize = 100;

pub async fn get_rpc_as_json(url: &str) -> Result<serde_json::value::Value, serde_json::error::Error> {
    let res = reqwest::get(url)
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
}

#[tokio::main]
async fn main() {
    let url = env::args().nth(1)
        .unwrap_or("server:13031/v2/p2p".to_string());
    let count = env::args().nth(2)
        .unwrap_or("2".to_string())
        .parse::<usize>().unwrap();
    let value = get_rpc_as_json(&url).await
        .unwrap();
    if let serde_json::Value::Array(values) = value {
        assert_eq!(values.len(), count);
    }
}