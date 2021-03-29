// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, convert::TryFrom};
use serde::{Deserialize, Serialize, ser::{self, SerializeSeq, SerializeStruct}};
use storage::persistent::{KeyValueSchema, Encoder, Decoder, SchemaError};
use super::common::Initiator;

#[derive(Debug, Clone, Default)]
pub struct Comments {
    pub incoming_wrong_pow: Option<f64>,
    pub incoming_too_short: Option<usize>,
    pub incoming_uncertain: bool,
    pub outgoing_wrong_pow: Option<f64>,
    pub outgoing_too_short: Option<usize>,
    pub outgoing_uncertain: bool,
    pub outgoing_wrong_pk: bool,
}

impl Comments {
    fn ser(&self) -> ([u8; 6], [u8; 6]) {
        let mut i = [0; 6];
        i[0] = self.incoming_wrong_pow.as_ref().cloned().unwrap_or(0.0) as u8;
        i[1] = self.incoming_too_short.as_ref().cloned().unwrap_or(255) as u8;
        i[2] = if self.incoming_uncertain { 1 } else { 0 };
        let mut o = [0; 6];
        o[0] = self.outgoing_wrong_pow.as_ref().cloned().unwrap_or(0.0) as u8;
        o[1] = self.outgoing_too_short.as_ref().cloned().unwrap_or(255) as u8;
        o[2] = if self.outgoing_uncertain { 1 } else { 0 };
        o[3] = if self.outgoing_wrong_pk { 1 } else { 0 };

        (i, o)
    }

    fn de((i, o): ([u8; 6], [u8; 6])) -> Self {
        Comments {
            incoming_wrong_pow: if i[0] == 0 { None } else { Some(i[0] as f64) },
            incoming_too_short: if i[1] == 255 { None } else { Some(i[1] as usize )},
            incoming_uncertain: i[2] != 0,
            outgoing_wrong_pow: if o[0] == 0 { None } else { Some(o[0] as f64) },
            outgoing_too_short: if o[1] == 255 { None } else { Some(o[1] as usize )},
            outgoing_uncertain: o[2] != 0,
            outgoing_wrong_pk: o[3] != 0,
        }
    }
}

impl Serialize for Comments {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut s = serializer.serialize_seq(None)?;
        if let Some(target) = self.incoming_wrong_pow {
            let msg = format!("incoming connection message bad proof-of-work, target: {}", target);
            s.serialize_element(&msg)?;
        }
        if let Some(size) = self.incoming_too_short {
            let msg = format!("incoming connection message is too short: {} bytes", size);
            s.serialize_element(&msg)?;
        }
        if self.incoming_uncertain {
            let msg = "incoming data does not look like a connection message";
            s.serialize_element(&msg)?;
        }
        if let Some(target) = self.outgoing_wrong_pow {
            let msg = format!("outgoing connection message bad proof-of-work, target: {}", target);
            s.serialize_element(&msg)?;
        }
        if let Some(size) = self.outgoing_too_short {
            let msg = format!("outgoing connection message is too short: {} bytes", size);
            s.serialize_element(&msg)?;
        }
        if self.outgoing_uncertain {
            let msg = "outgoing data does not look like a connection message";
            s.serialize_element(&msg)?;
        }
        if self.outgoing_wrong_pk {
            let msg = "outgoing connection message public key does not match with identity";
            s.serialize_element(&msg)?;
        }

        s.end()
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub ts: u64,
    pub ts_nanos: u32,
    pub initiator: Initiator,
    pub remote_addr: SocketAddr,
    peer_pk: [u8; 32],
    comments: Comments,
}

impl Item {
    pub fn new(initiator: Initiator, remote_addr: SocketAddr) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let ts = (timestamp / 1_000_000_000) as u64;
        let ts_nanos = (timestamp % 1_000_000_000) as u32;

        Item {
            ts,
            ts_nanos,
            initiator,
            remote_addr,
            peer_pk: [0; 32],
            comments: Comments::default(),
        }
    }

    pub fn set_peer_pk(&mut self, peer_pk: [u8; 32]) {
        self.peer_pk = peer_pk;
    }

    pub fn add_comment(&mut self) -> &mut Comments {
        &mut self.comments
    }

    #[rustfmt::skip]
    pub fn split(self) -> (Key, Value) {
        let Item { ts, ts_nanos, initiator, remote_addr, peer_pk, comments } = self;
        (Key { ts, ts_nanos }, Value { initiator, remote_addr, peer_pk, comments })
    }

    #[rustfmt::skip]
    pub fn unite(key: Key, value: Value) -> Self {
        let (Key { ts, ts_nanos }, Value { initiator, remote_addr, peer_pk, comments }) = (key, value);
        Item { ts, ts_nanos, initiator, remote_addr, peer_pk, comments }
    }

    pub fn key(&self) -> Key {
        Key {
            ts: self.ts,
            ts_nanos: self.ts_nanos,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    pub ts: u64,
    pub ts_nanos: u32,
}

impl Encoder for Key {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut v = Vec::with_capacity(12);
        v.extend_from_slice(&self.ts.to_be_bytes());
        v.extend_from_slice(&self.ts_nanos.to_be_bytes());
        Ok(v)
    }
}

impl Decoder for Key {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 12 {
            return Err(SchemaError::DecodeError);
        }

        Ok(Key {
            ts: u64::from_be_bytes(TryFrom::try_from(&bytes[..8]).unwrap()),
            ts_nanos: u32::from_be_bytes(TryFrom::try_from(&bytes[8..]).unwrap()),
        })
    }
}

// ip 16 bytes, port 2 bytes, initiator 1 byte, padding 1 byte, comments 12 bytes, peer_pk 32 bytes
pub struct Value {
    initiator: Initiator,
    remote_addr: SocketAddr,
    peer_pk: [u8; 32],
    comments: Comments,
}

impl Encoder for Value {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        use std::net::IpAddr;

        let mut v = Vec::with_capacity(64);

        let ip = match self.remote_addr.ip() {
            IpAddr::V4(ip) => ip.to_ipv6_mapped().octets(),
            IpAddr::V6(ip) => ip.octets(),
        };
        v.extend_from_slice(&ip);
        v.extend_from_slice(&self.remote_addr.port().to_le_bytes());

        v.push(if self.initiator.incoming() { 1 } else { 0 });
        v.push(0);

        let (i, o) = self.comments.ser();
        v.extend_from_slice(&i);
        v.extend_from_slice(&o);

        v.extend_from_slice(&self.peer_pk);

        Ok(v)
    }
}

impl Decoder for Value {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 64 {
            return Err(SchemaError::DecodeError);
        }

        Ok(Value {
            initiator: Initiator::new(bytes[18] != 0),
            remote_addr: {
                let ip = <[u8; 16]>::try_from(&bytes[0..16]).unwrap();
                let port = u16::from_le_bytes(TryFrom::try_from(&bytes[16..18]).unwrap());
                (ip, port).into()
            },
            peer_pk: TryFrom::try_from(&bytes[32..64]).unwrap(),
            comments: {
                let i = TryFrom::try_from(&bytes[20..26]).unwrap();
                let o = TryFrom::try_from(&bytes[26..32]).unwrap();
                Comments::de((i, o))
            },
        })
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use crypto::{blake2b, hash::HashType};

        let calc_peer_id = || -> Result<String, String> {
            let hash = blake2b::digest_128(&self.peer_pk).map_err(|e| e.to_string())?;
            HashType::CryptoboxPublicKeyHash
                .hash_to_b58check(&hash)
                .map_err(|e| e.to_string())
        };
        let peer_id = match calc_peer_id() {
            Ok(s) => s,
            Err(s) => s,
        };

        let mut s = serializer.serialize_struct("Connection", 3)?;
        s.serialize_field("initiator", &self.initiator)?;
        s.serialize_field("remote_addr", &self.remote_addr)?;
        s.serialize_field("peer_id", &peer_id)?;
        s.end()
    }
}

pub struct Schema;

impl KeyValueSchema for Schema {
    type Key = Key;
    type Value = Value;

    fn name() -> &'static str {
        "connection_storage"
    }
}
