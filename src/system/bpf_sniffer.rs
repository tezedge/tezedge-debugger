use std::{convert::TryFrom, net::{IpAddr, SocketAddr}};
use tokio::stream::StreamExt;
use sniffer::facade::{Module, SnifferEvent};

use super::SystemSettings;

pub fn build_bpf_sniffing_system(settings: SystemSettings) {
    tokio::spawn(async move {
        let (module, _events) = Module::load();
        let mut events = module.main_buffer();
        while let Some(event) = events.next().await {
            match SnifferEvent::try_from(event.as_ref()) {
                Err(e) => tracing::error!("{:?}", e),
                Ok(SnifferEvent::Write { .. }) => {
                    //tracing::info!("Write: id: {:?}, data: {:?}", id, data.len());
                },
                Ok(SnifferEvent::Read { .. }) => {
                    //tracing::info!("Read: id: {:?}, data: {:?}", id, data.len());
                },
                Ok(SnifferEvent::Connect { id, address }) => {
                    tracing::info!("Connect: id: {:?}, address: {}", id, address);
                    if should_ignore(&settings, &address) {
                        module.ignore(id.fd);
                        tracing::info!("Ignoring: id: {:?}, address: {}", id, address);
                    }
                },
                Ok(SnifferEvent::LocalAddress { id, address }) => {
                    tracing::info!("LocalAddress: id: {:?}, address: {}", id, address);
                },
                Ok(SnifferEvent::Close { id }) => {
                    tracing::info!("Close: id: {:?}", id);
                },
            }
        }
    });
}

fn should_ignore(settings: &SystemSettings, address: &SocketAddr) -> bool {
    match address.port() {
        0 | 65535 => {
            return true;
        },
        // dns
        53 => {
            return true;
        },
        // our
        p if p == settings.syslog_port || p == settings.rpc_port => {
            return true;
        },
        _ => (),
    }
    // lo v6
    if address.ip() == "::1".parse::<IpAddr>().unwrap() {
        return true;
    }
    // lo v4
    if address.ip() == "127.0.0.1".parse::<IpAddr>().unwrap() {
        return true;
    }

    return false;
}
