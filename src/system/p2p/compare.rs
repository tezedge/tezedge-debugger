// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

// incomplete

use std::{fmt, string::ToString, ops::{Add, Sub}};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize, ser, de};
use crate::messages::p2p_message::FullPeerMessage;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Peer {
    pub peer_id: String,
    created: DateTime<Utc>,
    pub peer_metadata: PeerMetadata,
    #[serde(default)] 
    events: Vec<Event>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PeerMetadata {
    responses: CountsByGroups,
    requests: CountsByGroups,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    valid_blocks: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    old_heads: i64,
    //prevalidator_results: (),
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    unactivated_chains: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    inactive_chains: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    future_blocks_advertised: i64,
    //unadvertised: (),
    //advertisements: (),
}

impl<'a, 'b> Sub<&'b PeerMetadata> for &'a PeerMetadata {
    type Output = PeerMetadata;

    fn sub(self, rhs: &'b PeerMetadata) -> Self::Output {
        PeerMetadata {
            responses: &self.responses - &rhs.responses,
            requests: &self.requests - &rhs.requests,
            valid_blocks: self.valid_blocks - rhs.valid_blocks,
            old_heads: self.old_heads - rhs.old_heads,
            unactivated_chains: self.unactivated_chains - rhs.unactivated_chains,
            inactive_chains: self.inactive_chains - rhs.inactive_chains,
            future_blocks_advertised: self.future_blocks_advertised - rhs.future_blocks_advertised,
        }
    }
}

impl PeerMetadata {
    pub fn count_message(&mut self, message: &FullPeerMessage, incoming: bool) {
        match message {
            &FullPeerMessage::Disconnect => (),
            &FullPeerMessage::Advertise(_) => (),
            &FullPeerMessage::SwapRequest(_) => (),
            &FullPeerMessage::SwapAck(_) => (),
            &FullPeerMessage::Bootstrap => (),
            &FullPeerMessage::GetCurrentBranch(_) => self.requests.get_mut(incoming).branch += 1,
            &FullPeerMessage::CurrentBranch(_) => self.responses.get_mut(incoming).branch += 1,
            &FullPeerMessage::Deactivate(_) => (),
            &FullPeerMessage::GetCurrentHead(_) => self.requests.get_mut(incoming).head += 1,
            &FullPeerMessage::CurrentHead(_) => self.responses.get_mut(incoming).head += 1,
            &FullPeerMessage::GetBlockHeaders(_) => self.requests.get_mut(incoming).block_header += 1,
            &FullPeerMessage::BlockHeader(_) => self.responses.get_mut(incoming).block_header += 1,
            &FullPeerMessage::GetOperations(_) => self.requests.get_mut(incoming).operations += 1,
            &FullPeerMessage::Operation(_) => self.responses.get_mut(incoming).operations += 1,
            &FullPeerMessage::GetProtocols(_) => self.requests.get_mut(incoming).protocols += 1,
            &FullPeerMessage::Protocol(_) => self.responses.get_mut(incoming).protocols += 1,
            &FullPeerMessage::GetOperationHashesForBlocks(_) => self.requests.get_mut(incoming).operation_hashes_for_block += 1,
            &FullPeerMessage::OperationHashesForBlock(_) => self.responses.get_mut(incoming).operation_hashes_for_block += 1,
            &FullPeerMessage::GetOperationsForBlocks(_) => self.requests.get_mut(incoming).operations_for_block += 1,
            &FullPeerMessage::OperationsForBlocks(_) => self.responses.get_mut(incoming).operations_for_block += 1,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CountsByGroups {
    sent: CountsByKind,
    received: CountsByKind,
    failed: CountsByKind,
    #[serde(default)] 
    scheduled: CountsByKind,
}

impl<'a, 'b> Sub<&'b CountsByGroups> for &'a CountsByGroups {
    type Output = CountsByGroups;

    fn sub(self, rhs: &'b CountsByGroups) -> Self::Output {
        CountsByGroups {
            sent: &(&self.sent + &self.scheduled) - &(&rhs.sent + &rhs.scheduled),
            received: &self.received - &rhs.received,
            failed: &self.failed - &rhs.failed,
            scheduled: Default::default(),
        }
    }
}

impl CountsByGroups {
    fn get_mut(&mut self, incoming: bool) -> &mut CountsByKind {
        if incoming {
            &mut self.received
        } else {
            &mut self.sent
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CountsByKind {
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    branch: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    head: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    block_header: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    operations: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    protocols: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    operation_hashes_for_block: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    operations_for_block: i64,
    #[serde(serialize_with = "parse_int_ser")]
    #[serde(deserialize_with = "parse_int_de")]
    other: i64,
}

impl<'a, 'b> Add<&'b CountsByKind> for &'a CountsByKind {
    type Output = CountsByKind;

    fn add(self, rhs: &'b CountsByKind) -> Self::Output {
        CountsByKind {
            branch: self.branch + rhs.branch,
            head: self.head + rhs.head,
            block_header: self.block_header + rhs.block_header,
            operations: self.operations + rhs.operations,
            protocols: self.protocols + rhs.protocols,
            operation_hashes_for_block: self.operation_hashes_for_block + rhs.operation_hashes_for_block,
            operations_for_block: self.operations_for_block + rhs.operations_for_block,
            other: self.other + rhs.other,
        }
    }
}

impl<'a, 'b> Sub<&'b CountsByKind> for &'a CountsByKind {
    type Output = CountsByKind;

    fn sub(self, rhs: &'b CountsByKind) -> Self::Output {
        CountsByKind {
            branch: self.branch - rhs.branch,
            head: self.head - rhs.head,
            block_header: self.block_header - rhs.block_header,
            operations: self.operations - rhs.operations,
            protocols: self.protocols - rhs.protocols,
            operation_hashes_for_block: self.operation_hashes_for_block - rhs.operation_hashes_for_block,
            operations_for_block: self.operations_for_block - rhs.operations_for_block,
            other: self.other - rhs.other,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {

}

fn parse_int_ser<S>(value: &i64, ser: S) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    ser.serialize_str(&value.to_string())
}

fn parse_int_de<'de, D>(de: D) -> Result<i64, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct V;

    impl<'de> de::Visitor<'de> for V {
        type Value = i64;
    
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("decimal representation as string")
        }
    
        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            s.parse().map_err(de::Error::custom)
        }
    }

    de.deserialize_any(V)
}
