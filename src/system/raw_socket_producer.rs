use crate::messages::tcp_packet::Packet;
use tracing::{trace, warn, error};
use std::{io};
use crate::system::orchestrator::spawn_packet_orchestrator;
use std::process::exit;

pub fn raw_socket_producer() -> io::Result<()> {
    use pnet::packet::ip::IpNextHeaderProtocols;
    use pnet::packet::{
        Packet as _,
    };
    use pnet::transport::{
        transport_channel, tcp_packet_iter,
        TransportChannelType,
    };

    let (_, mut recv) = transport_channel(
        // 64KB == Largest possible TCP packet, this *CAN* and *SHOULD* be lower, as the packet size
        // is limited by lower layers protocols *NOT* by TCP packet limit.
        64 * 1024,
        // We want only valid TCP headers with including IP headers
        TransportChannelType::Layer3(IpNextHeaderProtocols::Tcp),
    )?;

    let orchestrator = spawn_packet_orchestrator();
    std::thread::spawn(move || {
        let mut packet_iter = tcp_packet_iter(&mut recv);
        loop {
            let capture = packet_iter.next();
            match capture {
                Ok((packet, _)) => {
                    let packet = if let Some(packet) = Packet::new(packet.packet()) {
                        trace!(captured_length = packet.ip_buffer().len(), "captured packet");
                        packet
                    } else {
                        warn!(packet = debug(packet), "received invalid tcp packet");
                        continue;
                    };

                    loop {
                        match orchestrator.send(packet) {
                            Ok(()) => {
                                trace!("sent packet for processing");
                                break;
                            }
                            Err(_) => {
                                error!("orchestrator channel closed abruptly");
                                exit(-1);
                            }
                        }
                    }
                }
                Err(err) => {
                    warn!(error = display(err), "failed capture packet from socket");
                }
            }
        }
    });

    Ok(())
}