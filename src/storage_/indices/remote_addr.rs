use std::net::{SocketAddr, IpAddr};
use storage::persistent::{KeyValueSchema, Decoder, SchemaError, Encoder};
use super::FilterField;

impl<Schema> FilterField<Schema> for SocketAddr
where
    Schema: KeyValueSchema<Key = u64>,
{
    type Key = RemoteAddrKey;

    fn make_index(&self, primary_key: &<Schema as KeyValueSchema>::Key) -> Self::Key {
        let ip = match self.ip() {
            IpAddr::V4(ip) => ip.to_ipv6_mapped(),
            IpAddr::V6(ip) => ip,
        };
        RemoteAddrKey {
            addr: u128::from_be_bytes(ip.octets()),
            port: self.port(),
            index: primary_key.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteAddrKey {
    addr: u128,
    port: u16,
    index: u64,
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Decoder for RemoteAddrKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 26 {
            return Err(SchemaError::DecodeError);
        }

        Ok(RemoteAddrKey {
            addr: {
                let mut b = [0; 16];
                b.clone_from_slice(&bytes[0..16]);
                u128::from_be_bytes(b)
            },
            port: {
                let mut b = [0; 2];
                b.clone_from_slice(&bytes[16..18]);
                u16::from_be_bytes(b)
            },
            index: {
                let mut b = [0; 8];
                b.clone_from_slice(&bytes[18..26]);
                u64::from_be_bytes(b)
            },
        })
    }
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Encoder for RemoteAddrKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(26);
        buf.extend_from_slice(&self.addr.to_be_bytes());
        buf.extend_from_slice(&self.port.to_be_bytes());
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 26 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}
