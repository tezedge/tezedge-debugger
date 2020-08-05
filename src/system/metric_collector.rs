// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::system::SystemSettings;

/// infinitely performs http requests to cadvisor and put response into db
pub async fn metric_collector(settings: SystemSettings) {
    use tokio::{time, stream::StreamExt};
    use reqwest::Url;
    use tracing::error;
    use chrono::Duration;
    use std::{fmt, collections::HashMap};
    use crate::{
        messages::metric_message::{MetricMessage, ContainerInfo},
        storage::MetricStore,
        system::{
            notification::{Sender, SendError, NotificationMessage},
            metric_alert::SystemCapacityObserver,
        },
        utility::docker::DockerClient,
    };

    enum MetricCollectionError {
        Reqwest(reqwest::Error),
        Io(reqwest::Error),
        DeserializeJson(serde_json::Error),
        StoreMessage(storage::StorageError),
        NotificationSend(SendError),
    }
    
    impl fmt::Display for MetricCollectionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use MetricCollectionError::*;
    
            match self {
                &Reqwest(ref e) => write!(f, "error performing http request: {}", e),
                &Io(ref e) => write!(f, "io error during http request: {}", e),
                &DeserializeJson(ref e) => write!(f, "failed to deserialize json: {}", e),
                &StoreMessage(ref e) => write!(f, "failed to store message in database: {}", e),
                &NotificationSend(ref e) => write!(f, "failed to send notification: {}", e),
            }
        }
    }

    fn notify_and_store(
        messages: Vec<MetricMessage>,
        storage: &MetricStore,
        observer: &mut SystemCapacityObserver,
        notifier: &mut Option<Sender>,
    ) -> Result<(), MetricCollectionError> {
        for message in messages.iter() {
            observer.observe(message);
        }

        // if observer has some alert and we have some notifier, send the notification
        if let Some(notifier) = notifier {
            let alert = observer.alert();
            if !alert.is_empty() {
                let message = alert.into_iter().fold(String::new(), |s, item| format!("{}{}\n", s, item));
                notifier.send(&NotificationMessage::Warning(message))
                    .map_err(MetricCollectionError::NotificationSend)?;
            }
            let status = observer.status();
            if !status.is_empty() {
                let message = status.into_iter().fold(String::new(), |s, item| format!("{}{}\n", s, item));
                notifier.send(&NotificationMessage::Info(message))
                    .map_err(MetricCollectionError::NotificationSend)?;
            }
        }
        // write into db
        storage
            .store_message_array(messages)
            .map_err(MetricCollectionError::StoreMessage)?;
        
        Ok(())
    }

    async fn fetch_cadvisor(url: &Url) -> Result<Vec<MetricMessage>, MetricCollectionError> {
        // perform http GET request 
        let r = reqwest::get(url.clone())
            .await
            .map_err(MetricCollectionError::Reqwest)?
            .text()
            .await
            .map_err(MetricCollectionError::Io)?;
        // deserialize the response as a json object, assume it is `ContainerInfo` map
        let info = serde_json::from_str::<HashMap<String, ContainerInfo>>(r.as_str())
            .map_err(MetricCollectionError::DeserializeJson)?;

        // find the first container that contains tezos node, assume there is single such container
        let messages = if let Some(container_info) = info.into_iter().find(|&(_, ref i)| i.tezos_node()) {
            // take stats from the `ContainerInfo` object, wrap it as `MetricMessage`
            // and show it to `SystemCapacityObserver` in order to determine if should show an alert
            container_info.1.stats
                .into_iter()
                .map(|x| {
                    let message = MetricMessage::Cadvisor(x);
                    message
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(messages)
    }

    // login to messenger, it will provide object that can send alerts
    let messenger = settings
        .notification_cfg
        .channel
        .clone()
        .and_then(|config| {
            config
                .notifier()
                .map_err(|e|
                    error!(error = tracing::field::display(&e), "failed to login to slack")
                )
                .ok()
        });
    let mut sender = messenger.map(|m| m.sender(settings.notification_cfg.minimal_interval));
    let mut condition = settings.notification_cfg.alert_config.condition_checker();

    match &settings.cadvisor_url {
        &Some(ref url) => {
            // prepare url to fetch statistics from docker containers
            // unwrap is safe because joining constant
            let url = url
                .join("api/v1.3/docker")
                .unwrap();

            tokio::spawn(async move {
                loop {
                    let messages = fetch_cadvisor(&url)
                        .await
                        .unwrap_or_else(|e| {
                            error!(error = tracing::field::display(&e), "failed to fetch metrics");
                            Vec::new()
                        });
                    notify_and_store(messages, settings.storage.metric(), &mut condition, &mut sender)
                        .unwrap_or_else(|e|
                            error!(error = tracing::field::display(&e), "failed to send notification and store metrics")
                        );

                    // this interval should be less equal to 
                    // `--housekeeping_interval` of the cadvisor in the docker-compose.*.yml config
                    let duration = settings.metrics_fetch_interval
                        .to_std()
                        .unwrap_or_else(|e| {
                            error!(error = tracing::field::display(&e), "bad config value `metrics_fetch_interval`");
                            Duration::minutes(1).to_std().unwrap()
                        });
                    time::delay_for(duration).await;
                }
            });
        },
        None => {
            tokio::spawn(async move {
                match DockerClient::default().await {
                    Ok(mut client) => {
                        let list = client.list_containers().await.unwrap();
                        if let Some(container) = list.into_iter().find(|c| c.tezos_node()) {
                            let mut stats_stream = client.stats(container.id.as_str()).await;
                            while let Some(n) = stats_stream.next().await {
                                let messages = vec![MetricMessage::Docker(n.unwrap())];
                                let storage = settings.storage.metric();
                                notify_and_store(messages, storage, &mut condition, &mut sender)
                                    .unwrap_or_else(|e|
                                        error!(error = tracing::field::display(&e), "failed to send notification and store metrics")
                                    );
                            }
                        }
                    },
                    Err(e) => {
                        error!(error = tracing::field::display(&e), "failed to connect to docker socket");
                    },
                }
            });
        },
    }
}
