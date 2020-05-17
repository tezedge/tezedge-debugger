use serde::{Serialize, Deserialize, Deserializer};
use std::collections::HashMap;
use storage::persistent::{Encoder, SchemaError, Decoder};
use lazy_static::lazy_static;
use regex::Captures;
use chrono::{Utc, TimeZone, Datelike};
use crate::storage::get_ts;

fn parse_month(month: &str) -> Option<u32> {
    match month {
        "Jan" | "January" => Some(1),
        "Feb" | "February" => Some(2),
        "Mar" | "March" => Some(3),
        "Apr" | "April" => Some(4),
        "May" => Some(5),
        "Jun" | "June" => Some(6),
        "Jul" | "July" => Some(7),
        "Aug" | "August" => Some(8),
        "Sep" | "September" => Some(9),
        "Oct" | "October" => Some(10),
        "Nov" | "November" => Some(11),
        "Dec" | "December" => Some(11),
        _ => None,
    }
}

fn parse_date(date: &str) -> Option<u128> {
    use regex::Regex;
    lazy_static! {
        static ref DATE_RE: Regex = Regex::new(
            r"^(?P<month>\w+) (?P<day>\d+) (?P<hour>\d+):(?P<minute>\d+):(?P<second>\d+)$"
        ).expect("Invalid regex format");
    }
    let captures: Captures = DATE_RE.captures(date)?;
    let month = parse_month(captures.name("month")?.as_str())?;
    let day = captures.name("day")?.as_str().parse().unwrap();
    let hour = captures.name("hour")?.as_str().parse().unwrap();
    let minute = captures.name("minute")?.as_str().parse().unwrap();
    let second = captures.name("second")?.as_str().parse().unwrap();
    let dt = Utc.ymd(Utc::now().year(), month, day).and_hms(hour, minute, second);
    Some(dt.timestamp_nanos() as u128)
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>
{
    if deserializer.is_human_readable() {
        Ok(parse_date(&String::deserialize(deserializer)?).unwrap())
    } else {
        u128::deserialize(deserializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub level: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: u128,
    pub section: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(rename = "loc-file", skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(rename = "loc-line", skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(rename = "loc-column", skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

impl LogMessage {
    pub fn raw(line: String) -> Self {
        let mut extra = HashMap::with_capacity(1);
        extra.insert("message".to_string(), line);
        Self {
            level: "fatal".to_string(),
            date: get_ts(),
            section: "".to_string(),
            id: None,
            file: None,
            line: None,
            column: None,
            extra,
        }
    }
}

impl Encoder for LogMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_cbor::to_vec(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

impl Decoder for LogMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_cbor::from_slice(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}
