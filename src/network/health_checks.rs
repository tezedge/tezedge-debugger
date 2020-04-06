use std::time::{Duration, Instant};
use failure::Fail;
use packet::ip::{
    Packet as IPPacket,
    v4::Packet as IPv4Packet,
    Protocol,
};
use tun::Device;
use std::process::Command;

#[derive(Debug, Clone, Fail)]
pub enum HealthCheckError {
    #[fail(display = "Did not received expected response from {} ({})", from, test)]
    NoResponse {
        from: String,
        test: String,
    },
    #[fail(display = "Failed to run the check because of incorrect tun setup: {}", reason)]
    IncorrectDeviceSetup {
        reason: String,
    },
    #[fail(display = "Hit timeout while {}", activity)]
    TimeOut {
        activity: String,
    },
    #[fail(display = "{}: {}", detail, inner)]
    Comprehensive {
        detail: String,
        inner: Box<HealthCheckError>,
    },
}

impl HealthCheckError {
    pub fn incorrect_setup<T: ToString>(reason: T) -> Self {
        HealthCheckError::IncorrectDeviceSetup { reason: reason.to_string() }
    }
    pub fn timeout<T: ToString>(activity: T) -> Self {
        HealthCheckError::TimeOut { activity: activity.to_string() }
    }
    pub fn descriptive<T: ToString>(detail: T, reason: Self) -> Self {
        HealthCheckError::Comprehensive { detail: detail.to_string(), inner: Box::new(reason) }
    }
}

fn get_next_packet<T: Device>(dev: &mut T, protocol: Protocol, timeout: Duration) -> Result<IPv4Packet<Vec<u8>>, HealthCheckError> {
    let mut buf = [0u8; 65535];
    let now = Instant::now();
    loop {
        let read = dev.read(&mut buf).map_err(HealthCheckError::incorrect_setup)?;
        let data = &buf[0..read];
        if let Ok(packet) = IPPacket::new(data) {
            if let IPPacket::V4(ipv4_packet) = packet {
                if ipv4_packet.protocol() == protocol {
                    return Ok(ipv4_packet.to_owned());
                }
            }
        }

        if now.elapsed() > timeout {
            return Err(HealthCheckError::timeout(format!("waiting for {:?} packet", protocol)));
        }
    }
}

/// Top-level health check for testing correct setup of tun addresses
pub fn device_ping_check<T: 'static + Device + Send>(dev: T, addr: &str) -> Result<T, HealthCheckError> {
    let detail = format!("invalid setting detected on device {}", dev.name());
    let clone = detail.clone();
    // Address to ping
    log::info!("Checking address setting {} for {}", addr, dev.name());
    let handler = std::thread::spawn(move || -> Result<T, HealthCheckError> {
        let mut dev = dev;
        get_next_packet(&mut dev, Protocol::Icmp, Duration::from_secs(1))
            .map_err(|e| HealthCheckError::descriptive(clone, e))?;
        Ok(dev)
    });
    Command::new("ping")
        .args(&["-4", "-c 1", "-w 1", addr])
        .output()
        .unwrap();
    Ok(handler.join().unwrap()
        .map_err(|e| HealthCheckError::descriptive(detail.clone(), HealthCheckError::incorrect_setup(e)))?)
}