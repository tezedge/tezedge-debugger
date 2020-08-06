// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Top {
    #[serde(rename = "Processes")]
    pub processes: Vec<Vec<String>>,
    #[serde(rename = "Titles")]
    pub titles: Vec<String>,
}
