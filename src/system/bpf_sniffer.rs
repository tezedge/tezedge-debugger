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
                Ok(SnifferEvent::Write { fd, data }) => {
                    tracing::info!("Write: fd: {}, data: {:?}", fd, data.len());
                },
                Ok(SnifferEvent::Read { fd, data }) => {
                    tracing::info!("Read: fd: {}, data: {:?}", fd, data.len());
                },
                Ok(SnifferEvent::Connect { fd, address }) => {
                    tracing::info!("Connect: fd: {}, address: {}", fd, address);
                    if should_ignore(&settings, &address) {
                        module.ignore(fd);
                        tracing::info!("Ignoring: fd: {}, address: {}", fd, address);
                    }
                },
                Ok(SnifferEvent::Close { fd }) => {
                    tracing::info!("Close: fd: {}", fd);
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
