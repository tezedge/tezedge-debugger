// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Container {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Image")]
    pub image: String,
}

impl Container {
    pub fn tezos_node(&self) -> bool {
        self.image.starts_with("tezos/tezos")
    }
}
