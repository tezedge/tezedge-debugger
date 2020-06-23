use crate::messages::tcp_packet::Packet;
use tracing::{trace, error};
use std::{io, env, os::unix::io::AsRawFd};
use crate::system::orchestrator::spawn_packet_orchestrator;
use crate::system::SystemSettings;
use smoltcp::{
    time::Instant,
    wire::{EthernetFrame},
    phy::{RawSocket, Device, RxToken, wait},
};

pub fn raw_socket_producer(settings: SystemSettings) -> io::Result<()> {
    let orchestrator = spawn_packet_orchestrator(settings.clone());
    let ifname = env::args().nth(1)
        .unwrap_or("eth0".to_string());
    std::thread::spawn(move || {
        let mut packet_buf = [0u8; 64 * 1024];
        let mut socket = RawSocket::new(&ifname)
            .unwrap();
        loop {
            let _ = wait(socket.as_raw_fd(), None);
            if let Some((rx_token, _)) = socket.receive() {
                let packet_frame = rx_token.consume(Instant::now(), |buffer| {
                    (packet_buf[..buffer.len()]).clone_from_slice(buffer);
                    let data = &packet_buf[..buffer.len()];
                    let frame = EthernetFrame::new_unchecked(data);
                    Ok(frame.payload())
                }).unwrap();

                if let Some(packet) = Packet::new(packet_frame) {
                    match orchestrator.send(packet) {
                        Ok(_) => {
                            trace!("sent packet for processing");
                        }
                        Err(_) => {
                            error!("orchestrator channel closed abruptly");
                        }
                    }
                }
            }
        }
    });

    Ok(())
}