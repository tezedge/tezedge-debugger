#[tokio::main]
async fn main() {
    let types = "connection_message,metadata,ack_message,disconnect,advertise,swap_request,\
        swap_ack,bootstrap,get_current_branch,current_branch,deactivate,\
        get_current_head,current_head,get_block_headers,block_header,get_operations,operation\
        get_protocols,protocol,get_operation_hashes_for_blocks,operation_hashes_for_block,\
        get_operations_for_blocks,operations_for_blocks";

    #[derive(serde::Serialize)]
    struct Example<'a> {
        ty: &'a str,
        hex: String,
    }

    let mut examples = Vec::<Example>::new();
    for ty in types.split(',') {
        let url = format!("http://debug.dev.tezedge.com:17742/v3/messages?types={}&limit=1", ty);
        let list = reqwest::get(url)
            .await.unwrap()
            .text()
            .await.unwrap();
        let list = serde_json::from_str::<serde_json::Value>(&list).unwrap();

        if let Some(item) = list.as_array().and_then(|x| x.first()) {
            let id = item.as_object().unwrap().get("id").unwrap().as_u64().unwrap();
            let url = format!("http://debug.dev.tezedge.com:17742/v3/message/{}", id);
            let item = reqwest::get(url)
                .await.unwrap()
                .text()
                .await.unwrap();
            let item = serde_json::from_str::<serde_json::Value>(&item).unwrap();
            let o = item.as_object().unwrap().get("decrypted_bytes").unwrap().as_array().unwrap();
            let data = o.iter()
                .map(|i| {
                    let s = i.as_str().unwrap();
                    hex::decode(s).unwrap().into_iter()
                })
                .flatten()
                .collect::<Vec<_>>();
            examples.push(Example {
                ty,
                hex: hex::encode(&data),
            });
        } else {
            eprintln!("warning: no example for type: {}", ty);
        }
    }

    let json = serde_json::to_string(&examples).unwrap();
    println!("{}", json);
}
