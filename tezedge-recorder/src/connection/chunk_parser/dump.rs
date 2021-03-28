// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs::{self, File}, io::Write, marker::PhantomData};
use typenum::Bit;
use super::tables::connection;

pub struct Dump<S> {
    file: Option<File>,
    incoming: PhantomData<S>,
}

impl<S> Dump<S>
where
    S: Bit,
{
    pub fn new(cn: connection::Item) -> Self {
        let _ = fs::create_dir("target");
        Dump {
            file: {
                log::warn!("dump connection: {:?}", cn);
                let s = if S::BOOL { "incoming" } else { "outgoing" };
                File::create(format!("target/{}_{}", cn.id, s))
                    .map_err(|error| log::error!("cannot create dump for {}, {}", cn.id, error))
                    .ok()
            },
            incoming: PhantomData,
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
