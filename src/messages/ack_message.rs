use tezos_messages::p2p::binary_message::cache::{NeverCache, CachedData, CacheReader, CacheWriter};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, Tag, TagMap};
use serde::{Deserialize, Serialize};

use std::fmt;
use std::mem::size_of;
use storage::persistent::BincodeEncoded;


static DUMMY_BODY_CACHE: NeverCache = NeverCache;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
/// Acknowledgment message as defined in the protocol
pub enum AckMessage {
    Ack,
    NackV0,
    Nack(NackInfo),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
/// Motive of rejection
pub enum NackMotive {
    NoMotive,
    TooManyConnections,
    UnknownChainName,
    DeprecatedP2pVersion,
    DeprecatedDistributedDbVersion,
    AlreadyConnected
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
/// Information about connection rejection
pub struct NackInfo {
    pub motive: NackMotive,
    pub potential_peers_to_connect: Vec<String>,
}

impl NackInfo {
    pub fn new(motive: NackMotive, potential_peers_to_connect: &[String]) -> Self {
        Self {
            motive,
            potential_peers_to_connect: potential_peers_to_connect.to_vec()
        }
    }
}

impl fmt::Debug for NackInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let motive = match &self.motive {
            NackMotive::NoMotive => "No_motive".to_string(),
            NackMotive::TooManyConnections => "Too_many_connections ".to_string(),
            NackMotive::UnknownChainName => "Unknown_chain_name".to_string(),
            NackMotive::DeprecatedP2pVersion => "Deprecated_p2p_version".to_string(),
            NackMotive::DeprecatedDistributedDbVersion => "Deprecated_distributed_db_version".to_string(),
            NackMotive::AlreadyConnected => "Already_connected".to_string(),
        };
        let potential_peers_to_connect = self.potential_peers_to_connect.join(", ");
        write!(f, "motive: {}, potential_peers_to_connect: {:?}", motive, potential_peers_to_connect)
    }
}

impl NackInfo {
    fn encoding() -> Encoding {
        Encoding::Obj(
            vec![
                Field::new("motive", Encoding::Tags(
                    size_of::<u16>(),
                    TagMap::new(&[
                        Tag::new(0, "NoMotive", Encoding::Unit),
                        Tag::new(1, "TooManyConnections", Encoding::Unit),
                        Tag::new(2, "UnknownChainName", Encoding::Unit),
                        Tag::new(3, "DeprecatedP2pVersion", Encoding::Unit),
                        Tag::new(4, "DeprecatedDistributedDbVersion", Encoding::Unit),
                        Tag::new(5, "AlreadyConnected", Encoding::Unit),
                    ]),
                )),
                Field::new("potential_peers_to_connect", Encoding::dynamic(Encoding::list(Encoding::String))),
            ]
        )
    }
}


impl HasEncoding for AckMessage {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Ack", Encoding::Unit),
                Tag::new(0x01, "Nack", NackInfo::encoding()),
                Tag::new(0xFF, "NackV0", Encoding::Unit),
            ]),
        )
    }
}

impl BincodeEncoded for AckMessage {}

impl CachedData for AckMessage {
    fn cache_reader(&self) -> &dyn CacheReader {
        &DUMMY_BODY_CACHE
    }

    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        None
    }
}