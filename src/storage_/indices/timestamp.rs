use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::FilterField;

impl<Schema> FilterField<Schema> for u128
where
    Schema: KeyValueSchema<Key = u64>,
{
    type Key = TimestampKey;

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        TimestampKey {
            timestamp: *self,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimestampKey {
    timestamp: u128,
    index: u64,
}

/// * bytes layout: `[timestamp(16)][index(8)]`
impl Decoder for TimestampKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 24 {
            return Err(SchemaError::DecodeError);
        }

        Ok(TimestampKey {
            timestamp: {
                let mut b = [0; 16];
                b.clone_from_slice(&bytes[0..16]);
                u128::from_be_bytes(b)
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[16..24]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[timestamp(16)][index(8)]`
impl Encoder for TimestampKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(24);
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 24 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
