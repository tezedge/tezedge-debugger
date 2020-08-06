// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::system::SystemSettings;

/// infinitely performs http requests to cadvisor and put response into db
pub async fn metric_collector(settings: SystemSettings) {
    use tokio::stream::StreamExt;
    use tracing::{error, warn};
    use std::fmt;
    use chrono::Utc;
    use crate::{
        messages::metric_message::MetricMessage,
        storage::MetricStore,
        utility::{
            docker::{DockerClient, Container, Stat},
            stats::{CapacityMonitor, Sender, SendError, NotificationMessage, StatSource},
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
        stat: Stat,
        storage: &MetricStore,
        observer: &mut CapacityMonitor,
        notifier: &mut Option<Sender>,
    ) -> Result<(), MetricCollectionError> {
        observer.observe(&stat);

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
            .store_message(MetricMessage(stat))
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

    tokio::spawn(async move {
        match DockerClient::default().await {
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
                    if let Some(container) = list.into_iter().find(Container::tezos_node) {
                        break container;
                    }
                    warn!(warning = "tezos node still not run, will retry in 5 seconds");
                    delay_for(Duration::new(5, 0)).await;
                };
                let mut stats_stream = client.stats(container.id.as_str()).await;
                let mut last_update = Utc::now();
                while let Some(stat) = stats_stream.next().await {
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
                    let delta = stat.timestamp() - last_update;
                    let num_quants = (delta.num_seconds() / settings.metrics_fetch_interval.num_seconds()) as i32;
                    if num_quants >= 1 {
                        last_update = last_update + settings.metrics_fetch_interval * num_quants;
                        let storage = settings.storage.metric();
                        notify_and_store(stat, storage, &mut monitor, &mut sender)
                            .unwrap_or_else(|e|
                                error!(
                                    error = tracing::field::display(&e),
                                    "failed to send notification and store metrics",
                                )
                            );
                    }
                }
            },
            Err(e) => {
                error!(error = tracing::field::display(&e), "failed to connect to docker socket");
            },
        }
    });
}
