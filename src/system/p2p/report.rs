use std::{net::SocketAddr, collections::HashMap};
use serde::{Serialize, Deserialize};
use tezos_messages::p2p::encoding::{version::NetworkVersion, metadata::MetadataMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerId(pub String);

#[allow(dead_code)]
pub type Connections = Vec<Connection>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    incoming: bool,
    peer_id: PeerId,
    id_point: SocketAddr,
    remote_socket_port: u16,
    announced_version: NetworkVersion,
    private: bool,
    local_metadata: MetadataMessage,
    remote_metadata: MetadataMessage,
}

#[allow(dead_code)]
pub type Peers = HashMap<PeerId, Peer>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub score: u32,
    pub trusted: bool,
    pub peer_metadata: PeerMetadata,
    pub state: PeerState,
    pub stat: PeerStat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMetadata {
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerState {
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStat {
    pub total_sent: u128,
    pub total_recv: u128,
    pub current_inflow: u32,
    pub current_outflow: u32,
}

#[allow(dead_code)]
pub type Points = HashMap<SocketAddr, Point>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub trusted: bool,
    pub state: PointState,
    pub last_established_connection: Option<(PeerId, Timestamp)>,
    pub last_seen: Option<(PeerId, Timestamp)>,
    pub greylisted_until: Option<Timestamp>,
    pub last_failed_connection: Option<Timestamp>,
    pub last_miss: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_kind", rename_all = "snake_case")]
pub enum PointState {
    Disconnected,
    Requested,
    Running {
        p2p_peer_id: PeerId,
    },
}

pub mod api {
    use serde::{Serialize, ser};

    pub struct Point(super::Point);

    impl Serialize for Point {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            let _ = serializer;
            unimplemented!()
        }
    }
}
