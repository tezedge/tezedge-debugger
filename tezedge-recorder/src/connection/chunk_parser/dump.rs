// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs::{self, File}, io::Write};
use super::tables::connection;

pub struct Dump {
    file: Option<File>,
}

impl Dump {
    pub fn new(cn: connection::Item) -> Self {
        let _ = fs::create_dir("target");
        Dump {
            file: {
                log::warn!("dump in file connection: {:?}", cn);
                File::create(format!("target/{}", cn.id))
                    .map_err(|error| log::error!("cannot create dump for {}, {}", cn.id, error))
                    .ok()
            },
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.file
            .as_mut()
            .and_then(|file| {
                file.write_all(data)
                    .map_err(|error| log::error!("cannot write dump: {}", error))
                    .ok()
            });
    }
}
