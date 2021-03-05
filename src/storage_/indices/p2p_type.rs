use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::{FilterField, Access};

pub struct P2pType(u32);

impl<Schema> FilterField<Schema> for P2pType
where
    Schema: KeyValueSchema<Key = u64>,
    Schema::Value: Access<P2pType>,
{
    type Key = P2pTypeKey;

    fn accessor(value: &<Schema as KeyValueSchema>::Value) -> Option<Self> {
        Some(value.accessor())
    }

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        P2pTypeKey {
            ty: self.0,
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct P2pTypeKey {
    ty: u32,
    index: u64,
}

/// * bytes layout: `[type(4)][padding(4)][index(8)]`
impl Decoder for P2pTypeKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 16 {
            return Err(SchemaError::DecodeError);
        }

        Ok(P2pTypeKey {
            ty: {
                let mut b = [0; 4];
                b.clone_from_slice(&bytes[0..4]);
                u32::from_be_bytes(b)
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[8..16]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[type(4)][padding(4)][index(8)]`
impl Encoder for P2pTypeKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf: Vec<u8> = Vec::with_capacity(16);
        buf.extend_from_slice(&self.ty.to_be_bytes());
        buf.extend_from_slice(&[0, 0, 0, 0]);
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 16 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
