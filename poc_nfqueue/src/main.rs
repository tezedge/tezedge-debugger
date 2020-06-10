use tokio::sync::mpsc::{Receiver, channel};
use std::net::IpAddr;

fn spawn_producer() -> Receiver<Vec<u8>> {
    use pnet::packet::ip::IpNextHeaderProtocols;
    use pnet::packet::{
        Packet,
        tcp::TcpPacket,
    };
    use pnet::transport::{
        transport_channel, tcp_packet_iter,
        TransportChannelType,
    };

    let (mut sender, receiver) = channel(1024);

    std::thread::spawn(move || {
        let (_, mut recv) = transport_channel(
            64 * 1024,
            TransportChannelType::Layer3(IpNextHeaderProtocols::Tcp),
        ).unwrap();
        let mut packet_iter = tcp_packet_iter(&mut recv);
        loop {
            let capture: Result<(TcpPacket, IpAddr), std::io::Error> = packet_iter.next();
            match capture {
                Ok((packet, _)) => {
                    let _ = sender.try_send(packet.packet().to_vec());
                }
                Err(err) => {
                    eprintln!("Failure occurred during packet reading: {}", err);
                }
            }
        }
    });
    receiver
}

#[tokio::main]
async fn main() {
    let mut packet_producer = spawn_producer();
    loop {
        if let Some(value) = packet_producer.recv().await {
            println!("Received new packet: {:?}", value);
        } else {
            panic!("Packet producing channel closed unexpectedly");
        }
    }
}
