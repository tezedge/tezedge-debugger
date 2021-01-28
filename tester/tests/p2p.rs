use std::env;

pub async fn get_p2p(params: &str) -> Result<serde_json::value::Value, serde_json::error::Error> {
    let debugger = env::var("DEBUGGER_URL")
        .unwrap();
    let res = reqwest::get(&format!("{}/?{}", debugger, params))
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
}

#[tokio::test]
async fn p2p_limit() {
    for limit in 0..8 {
        let response = get_p2p(&format!("limit={}", limit))
            .await.unwrap();
        assert_eq!(response.as_array().unwrap().len(), limit);
    }
}

#[tokio::test]
async fn p2p_cursor() {
    for cursor_id in 0..8 {
        let response = get_p2p(&format!("cursor_id={}", cursor_id))
            .await.unwrap();
        assert_eq!(response[0]["id"], cursor_id, "{}", cursor_id);
    }
}

#[tokio::test]
async fn p2p_types_filter() {
    let mut types = [
        ("connection_message", 0),
        ("metadata", 0),
        ("advertise", 0),
        ("get_block_headers", 0),
        ("block_header", 0),
    ];
    for &mut (ty, ref mut number) in &mut types {
        let response = get_p2p(&format!("limit=1000&types={}", ty))
            .await.unwrap();
        *number = response.as_array().unwrap().len();
    }

    // for all type combination
    for i in 0..(types.len() - 1) {
        let &(ty_i, n_ty_i) = &types[i];
        for j in (i + 1)..types.len() {
            let &(ty_j, n_ty_j) = &types[j];
            let response = get_p2p(&format!("limit=1000&types={},{}", ty_i, ty_j))
                .await.unwrap();
            assert_eq!(response.as_array().unwrap().len(), n_ty_i + n_ty_j);
        }
    }
}
