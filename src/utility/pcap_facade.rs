use pnet::datalink::{self, Channel, Config, DataLinkReceiver};

pub fn with_capture<F>(ifname: Option<String>, f: F)
where
    F: FnOnce(Box<dyn DataLinkReceiver>),
{
    let ifname = ifname.as_ref().map(String::as_str).unwrap_or("eth0");
    if let Some(interface) = datalink::interfaces().into_iter().find(|i| i.name == ifname) {
        // Create a channel to receive on
        let mut config = Config::default();
        config.read_buffer_size = 0x10000;
        config.write_buffer_size = 0x10000;
        match datalink::channel(&interface, config) {
            Ok(Channel::Ethernet(_, rx)) => {
                f(rx)
            }
            Ok(_) => tracing::warn!("packetdump: unhandled channel type"),
            Err(error) => tracing::error!(error = tracing::field::display(&error), "packetdump: unable to create channel"),
        };
    } else {
        tracing::error!(ifname = tracing::field::display(&ifname), "no such interface");
    }
}

/*
use pcap::{Device, Capture, Active};

pub fn with_capture<F>(ifname: Option<String>, f: F)
where
    F: FnOnce(Capture<Active>),
{
    let ifname = ifname.as_ref().map(String::as_str).unwrap_or("eth0");
    let device = Device::list()
        .unwrap_or_else(|error| {
            tracing::error!(error = tracing::field::display(&error), "pcap library error");
            Vec::new()
        })
        .into_iter()
        .find(|i| i.name == ifname);
    if let Some(device) = device {
        match device.open() {
            Ok(mut cap) => {
                cap.filter("tcp").unwrap_or_else(|error| {
                    tracing::error!(
                        error = tracing::field::display(&error),
                        "pcap library error, failed to filter",
                    );
                });
                f(cap)
            },
            Err(error) => {
                tracing::error!(
                    error = tracing::field::display(&error),
                    "pcap library error, failed to open device",
                );
            },
        }
    } else {
        tracing::error!(ifname = tracing::field::display(&ifname), "no such interface");
    }
}*/
