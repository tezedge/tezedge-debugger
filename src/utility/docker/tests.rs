use super::*;

#[tokio::test]
pub async fn docker_stat_stream() {
    use tokio::stream::StreamExt;

    let mut client = DockerClient::default().await.unwrap();
    let list = client.list_containers().await.unwrap();
    if let Some(container) = list.into_iter().find(|c| c.tezos_node()) {
        let mut s = client.stats(container.id.as_str()).await;
        while let Some(n) = s.next().await {
            let stat = n.unwrap();
            println!("{:#?}", stat);
        }
    }
}
