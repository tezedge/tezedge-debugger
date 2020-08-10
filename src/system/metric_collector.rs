// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::system::SystemSettings;

/// infinitely performs http requests to cadvisor and put response into db
pub async fn metric_collector(settings: SystemSettings) {
    use tracing::{error, warn};
    use std::fmt;
    use crate::{
        messages::metric_message::MetricMessage,
        storage::MetricStore,
        utility::{
            docker::{DockerClient, Container},
            stats::{CapacityMonitor, Sender, SendError, NotificationMessage, ProcessStat},
        },
    };

    enum MetricCollectionError {
        StoreMessage(storage::StorageError),
        NotificationSend(SendError),
    }
    
    impl fmt::Display for MetricCollectionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use MetricCollectionError::*;
    
            match self {
                &StoreMessage(ref e) => write!(f, "failed to store message in database: {}", e),
                &NotificationSend(ref e) => write!(f, "failed to send notification: {}", e),
            }
        }
    }

    fn notify_and_store(
        container: &Container,
        message: MetricMessage,
        storage: &MetricStore,
        observer: &mut CapacityMonitor,
        notifier: &mut Option<Sender>,
    ) -> Result<(), MetricCollectionError> {
        observer.observe(&message.container_stat);

        // if observer has some alert and we have some notifier, send the notification
        if let Some(notifier) = notifier {
            let container_info = format!("Container image: {}\n", container.image);
            let alert = observer.alert();
            if !alert.is_empty() {
                let message = alert.into_iter().fold(container_info.clone(), |s, item| format!("{}{}\n", s, item));
                notifier.send(&NotificationMessage::Warning(message))
                    .map_err(MetricCollectionError::NotificationSend)?;
            }
            let status = observer.status();
            if !status.is_empty() {
                let message = status.into_iter().fold(container_info, |s, item| format!("{}{}\n", s, item));
                notifier.send(&NotificationMessage::Info(message))
                    .map_err(MetricCollectionError::NotificationSend)?;
            }
        }
        // write into db
        storage
            .store_message(message)
            .map_err(MetricCollectionError::StoreMessage)?;
        
        Ok(())
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
                    error!(error = tracing::field::display(&e), "failed to login to messenger")
                )
                .ok()
        });
    let mut sender = messenger.map(|m| m.sender(settings.notification_cfg.minimal_interval));
    let mut monitor = settings.notification_cfg.alert_config.monitor();

    let client = match &settings.docker_daemon_address {
        &Some(ref addr) => DockerClient::connect(addr).await,
        &None => DockerClient::path("/var/run/docker.sock").await,
    };

    tokio::spawn(async move {
        match client {
            Ok(mut client) => {
                let container = loop {
                    use tokio::time::delay_for;
                    use std::time::Duration;
                    
                    let list = match client.list_containers().await {
                        Ok(list) => list,
                        Err(e) => {
                            warn!(
                                warning = tracing::field::display(&e),
                                "failed to fetch list of containers",
                            );
                            Vec::new()
                        },
                    };
                    if let Some(container) = list.into_iter().find(|c| c.image.starts_with(&settings.node_image_name)) {
                        break container;
                    }
                    warn!(warning = "tezos node still not run, will retry in 5 seconds");
                    delay_for(Duration::new(5, 0)).await;
                };
                loop {
                    let stat = client.stats_single(container.id.as_str()).await;
                    let stat = match stat {
                        Ok(stat) => stat,
                        Err(e) => {
                            warn!(
                                warning = tracing::field::display(&e),
                                "received bad stat, ignore",
                            );
                            continue
                        },
                    };
                    let process_stats = match client.top(container.id.as_str(), "u").await {
                        Ok(top) => ProcessStat::parse_top(top),
                        Err(err) => {
                            warn!(
                                warning = tracing::field::display(&err),
                                "failed to fetch top output from docker daemon",
                            );
                            vec![]
                        }
                    };
                    let message = MetricMessage {
                        container_stat: stat,
                        process_stats,
                    };
                    let storage = settings.storage.metric();
                    notify_and_store(&container, message, storage, &mut monitor, &mut sender)
                        .unwrap_or_else(|e|
                            error!(
                                error = tracing::field::display(&e),
                                "failed to send notification and store metrics",
                            )
                        );
                    tokio::time::delay_for(settings.metrics_fetch_interval.to_std().unwrap()).await;
                }
            },
            Err(e) => {
                error!(
                    error = tracing::field::display(&e),
                    "failed to connect to docker socket",
                );
            },
        }
    });
}
