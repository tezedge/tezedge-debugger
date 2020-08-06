use serde::{Serialize, Deserialize};
use storage::persistent::{Decoder, Encoder, SchemaError};
use chrono::{DateTime, Utc, TimeZone};
use crate::utility::{docker::Stat, stats::{StatSource, ProcessStat}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMessage {
    pub container_stat: Stat,
    pub process_stats: Vec<ProcessStat>,
}

impl MetricMessage {
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.container_stat.timestamp()
    }
}

#[derive(Debug, Clone)]
pub struct MetricMessageKey(pub DateTime<Utc>);

impl Decoder for MetricMessageKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        use byteorder::{ByteOrder, BigEndian};

        let t = BigEndian::read_i64(&bytes[..8]);
        Ok(MetricMessageKey(Utc.timestamp(t, 0)))
    }
}

impl Encoder for MetricMessageKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        use byteorder::{ByteOrder, BigEndian};

        let t = self.0.timestamp();
        let mut v = Vec::with_capacity(8);
        v.resize(8, 0);
        BigEndian::write_i64(v.as_mut(), t);
        Ok(v)
    }
}

impl Decoder for MetricMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for MetricMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}
