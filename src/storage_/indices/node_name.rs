use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::FilterField;

#[derive(Debug, Clone)]
pub struct NodeName(pub u16);

impl<Schema> FilterField<Schema> for NodeName
where
    Schema: KeyValueSchema<Key = u64>,
{
    type Key = NodeNameKey;

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        NodeNameKey {
            node_name: self.0,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeNameKey {
    node_name: u16,
    index: u64,
}

/// * bytes layout: `[node_name(2)][padding(6)][index(8)]`
impl Decoder for NodeNameKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(NodeNameKey {
            node_name: {
                let mut b = [0; 2];
                b.clone_from_slice(&bytes[0..2]);
                u16::from_be_bytes(b)
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[8..16]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[node_name(2)][padding(6)][index(8)]`
impl Encoder for NodeNameKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&self.node_name.to_be_bytes()); // is_remote_requested
        buf.extend_from_slice(&[0u8; 6]); // padding
        buf.extend_from_slice(&self.index.to_be_bytes()); // index

        if buf.len() != 16 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
