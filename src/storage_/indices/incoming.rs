use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::{FilterField, Access};

impl<Schema> FilterField<Schema> for bool
where
    Schema: KeyValueSchema<Key = u64>,
    Schema::Value: Access<bool>,
{
    type Key = IncomingKey;

    fn accessor(value: &<Schema as KeyValueSchema>::Value) -> Option<Self> {
        Some(value.accessor())
    }

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        IncomingKey {
            is_incoming: *self,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncomingKey {
    is_incoming: bool,
    index: u64,
}

/// * bytes layout: `[is_incoming(1)][padding(7)][index(8)]`
impl Decoder for IncomingKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(IncomingKey {
            is_incoming: match bytes[0] {
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

/// * bytes layout: `[is_incoming(1)][padding(7)][index(8)]`
impl Encoder for IncomingKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&[self.is_incoming as u8]); // is_incoming
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
