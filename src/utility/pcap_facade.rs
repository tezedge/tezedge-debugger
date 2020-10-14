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
            Ok(cap) => {
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
}
