use serde::{Serialize, Deserialize};
use crate::storage::get_ts;
use syslog_loose::Message;
use storage::persistent::BincodeEncoded;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub level: String,
    #[serde(alias = "timestamp", alias = "time", rename(serialize = "timestamp"))]
    pub date: u128,
    #[serde(alias = "module")]
    pub section: String,
    #[serde(alias = "msg", rename(serialize = "message"))]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl LogMessage {
    pub fn raw(line: String) -> Self {
        Self {
            level: "fatal".to_string(),
            date: get_ts(),
            section: "".to_string(),
            id: None,
            message: line,
        }
    }

    fn rust_log_line(line: &str) -> Option<(&str, &str)> {
        let (_, level_msg) = line.split_at(20);
        let level = level_msg.split_whitespace().next()?;
        let msg = &level_msg[level.len() + 1..];
        Some((level, msg))
    }

    fn ocaml_log_line(line: &str) -> Option<(&str, &str)> {
        let mut parts = line.split("-");
        let _ = parts.next();
        let msg = parts.next();
        if let Some(value) = msg {
            let mut parts = value.split(":");
            let _ = parts.next();
            let msg = parts.next();
            if let Some(msg) = msg {
                Some(("info", &msg[1..]))
            } else {
                Some(("info", &value[1..]))
            }
        } else {
            Some(("info", line))
        }
    }
}

impl<S: AsRef<str> + Ord + PartialEq + Clone> From<syslog_loose::Message<S>> for LogMessage {
    // <27>1 2020-06-24T10:32:37.026683+02:00 Ubuntu-1910-eoan-64-minimal 451e91e7df18 1482 451e91e7df18 - Jun 24 08:32:37.026 INFO Blacklisting IP because peer failed at bootstrap process, ip: 104.248.136.94
    fn from(msg: Message<S>) -> Self {
        let date = msg.timestamp
            .map(|dt| dt.timestamp_nanos() as u128)
            .unwrap_or_else(get_ts);
        let line = msg.msg.as_ref();

        let pos = line.find('.').unwrap_or_default();
        if pos == 15 {
            if let Some((level, message)) = Self::rust_log_line(line) {
                Self {
                    date,
                    level: level.to_string(),
                    message: message.to_string(),
                    section: "".to_string(),
                    id: None,
                }
            } else {
                Self {
                    date,
                    level: "fatal".to_string(),
                    section: "".to_string(),
                    id: None,
                    message: line.to_string(),
                }
            }
        } else {
            if let Some((level, message)) = Self::ocaml_log_line(line) {
                Self {
                    date,
                    level: level.to_string(),
                    message: message.to_string(),
                    section: "".to_string(),
                    id: None,
                }
            } else {
                Self {
                    date,
                    level: "fatal".to_string(),
                    section: "".to_string(),
                    id: None,
                    message: line.to_string(),
                }
            }
        }
    }
}

impl BincodeEncoded for LogMessage {}