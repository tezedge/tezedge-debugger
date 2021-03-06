use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use serde::{Serialize, Deserialize};
use super::{FilterField, Access};

/// Determines, if message belongs to communication originated
/// from remote or local node
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum SourceType {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "remote")]
    Remote,
}

impl SourceType {
    pub fn is_local(&self) -> bool {
        match self {
            &SourceType::Local => true,
            &SourceType::Remote => false,
        }
    }
}

impl<Schema> FilterField<Schema> for SourceType
where
    Schema: KeyValueSchema<Key = u64>,
    Schema::Value: Access<SourceType>,
{
    type Key = SourceTypeKey;

    fn accessor(value: &<Schema as KeyValueSchema>::Value) -> Option<Self> {
        Some(value.accessor())
    }

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        SourceTypeKey {
            source_type: self.is_local(),
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct SourceTypeKey {
    source_type: bool,
    index: u64,
}

/// * bytes layout: `[is_remote_requested(1)][padding(7)][index(8)]`
impl Decoder for SourceTypeKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(SourceTypeKey {
            source_type: match bytes[0] {
                0 => false,
                1 => true,
                _ => return Err(SchemaError::DecodeError),
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[8..16]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[is_remote_requested(1)][padding(7)][index(8)]`
impl Encoder for SourceTypeKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&[self.source_type as u8]); // is_remote_requested
        buf.extend_from_slice(&[0u8; 7]); // padding
        buf.extend_from_slice(&self.index.to_be_bytes()); // index

        if buf.len() != 16 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
