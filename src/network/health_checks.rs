use std::time::{Duration, Instant};
use failure::Fail;
use packet::ip::{
    Packet as IPPacket,
    v4::Packet as IPv4Packet,
    Protocol,
};
use tun::{
    Device as _,
    platform::Device,
};
use std::process::Command;
use timeout_readwrite::TimeoutReader;
use std::io::{Read, Write};
use std::net::Ipv4Addr;

#[derive(Debug, Fail)]
pub enum HealthCheckError {
    #[fail(display = "Did not received any response")]
    NoResponse,
    #[fail(display = "Failed to sent a packet: {}", inner)]
    SendFailed {
        inner: std::io::Error
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
    pub fn send_failed(inner: std::io::Error) -> Self {
        HealthCheckError::SendFailed { inner }
    }
}

fn get_next_packet(dev: &mut Device, protocol: Protocol, timeout: Duration) -> Result<IPv4Packet<Vec<u8>>, HealthCheckError> {
    let mut buf = [0u8; 65535];
    let now = Instant::now();
    let mut dev = TimeoutReader::new(dev, timeout);
    loop {
        let read = dev.read(&mut buf)
            .map_err(|_| HealthCheckError::NoResponse)?;
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
pub fn device_address_check(dev: Device, addr: &str) -> Result<Device, HealthCheckError> {
    let detail = format!("invalid setting detected on device {}", dev.name());
    // Address to ping
    log::info!("Checking address setting {} for {}", addr, dev.name());
    let handler = std::thread::spawn(move || {
        let mut dev = dev;
        get_next_packet(&mut dev, Protocol::Icmp, Duration::from_secs(1))?;
        Ok(dev)
    });
    Command::new("ping")
        .args(&["-4", "-c 1", "-w 1", addr])
        .output()
        .unwrap();
    Ok(handler.join().unwrap()
        .map_err(|e: HealthCheckError| HealthCheckError::descriptive(detail.clone(), e))?)
}

fn build_ping(addr: &str) -> Vec<u8> {
    use pnet::packet::{
        ipv4::{Ipv4Packet as PnetIpv4Packet, checksum}
    };

    let addr: Ipv4Addr = addr.parse().unwrap();
    let ping_packet = "45000054884f40004001dfdac0a801c7080808080800904c2fe300018c398c5e000000005864080000000000101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f3031323334353637".to_string();
    let mut ping_packet = hex::decode(ping_packet).unwrap();
    for (x, y) in (&mut ping_packet[12..16]).iter_mut().zip(addr.octets().iter()) {
        *x = *y;
    }
    let packet = PnetIpv4Packet::new(&ping_packet).unwrap();
    let ip_checksum: u16 = checksum(&packet);
    drop(packet);
    for (x, y) in (&mut ping_packet[10..12]).iter_mut().zip(ip_checksum.to_be_bytes().iter()) {
        *x = *y;
    }
    ping_packet
}

/// Top-level health check for testing if internet connection is reachable
pub fn internet_accessibility_check(dev: Device, addr: &str) -> Result<Device, HealthCheckError> {
    let detail = format!("unable to access internet (check FAQ for more info)");
    let addr = addr.to_string();
    log::info!("Checking internet accessibility on {}", dev.name());
    let handler = std::thread::spawn(move || {
        let mut dev = dev;
        let addr = addr;
        let packet = build_ping(&addr);
        dev.write_all(&packet)
            .map_err(HealthCheckError::send_failed)?;
        get_next_packet(&mut dev, Protocol::Icmp, Duration::from_secs(1))?;
        Ok(dev)
    });
    Ok(handler.join().unwrap()
        .map_err(|e: HealthCheckError| HealthCheckError::descriptive(detail.clone(), e))?)
}