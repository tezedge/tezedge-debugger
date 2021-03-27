// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs, io::Write};
use super::tables::connection;

pub struct Buffer {
    counter: u64,
    buffer: Vec<u8>,
    broken: Option<fs::File>,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            counter: 0,
            buffer: Vec::with_capacity(0x10000),
            broken: None,
        }
    }
}

impl Buffer {
    pub fn handle_data(&mut self, payload: &[u8], cn: &connection::Item) {
        if self.have_chunk().is_some() && self.broken.is_none() {
            log::warn!(
                "append new data while not consumed chunk, buffer len: {}, counter: {}",
                self.buffer.len(),
                self.counter,
            );
            
            let _ = fs::create_dir("target");
            let mut f = fs::File::create(format!("target/{}.ron", cn.id)).unwrap();
            f.write_fmt(format_args!("{:?}\n", cn)).unwrap();
            let mut f = fs::File::create(format!("target/{}", cn.id)).unwrap();
            f.write_all(&self.buffer).unwrap();
            self.buffer.clear();
            self.broken = Some(f);
        }
        if let Some(f) = &mut self.broken {
            f.write_all(payload).unwrap();
        } else {
            self.buffer.extend_from_slice(payload);
        }
    }

    pub fn remaining(&self) -> usize {
        self.buffer.len()
    }

    // length (including 2 bytes header) of the chunk at offset
    fn len(&self, offset: usize) -> Option<usize> {
        use std::convert::TryFrom;

        if self.buffer.len() >= offset + 2 {
            let b = <[u8; 2]>::try_from(&self.buffer[offset..2]).unwrap();
            Some(u16::from_be_bytes(b) as usize + 2)
        } else {
            None
        }
    }

    pub fn have_chunk(&self) -> Option<&[u8]> {
        let len = self.len(0)?;
        if self.buffer.len() >= len {
            Some(&self.buffer[..len])
        } else {
            None
        }
    }

    pub fn cleanup(&mut self) -> (u64, Vec<u8>) {
        use std::mem;

        let counter = self.counter;
        self.counter += 1;
        (counter, mem::replace(&mut self.buffer, Vec::new()))
    }
}

impl Iterator for Buffer {
    type Item = (u64, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.len(0)?;
        if self.buffer.len() < len {
            None
        } else {
            let counter = self.counter;
            self.counter += 1;

            let mut new = vec![0; len];
            new.copy_from_slice(&self.buffer[..len]);
            assert!(self.buffer.as_ptr() as usize != new.as_ptr() as usize);

            if self.buffer.len() > len {
                let mut remaining = vec![0; self.buffer.len() - len];
                remaining.copy_from_slice(&self.buffer[len..]);
                self.buffer.clear();
                self.buffer = remaining;
            } else {
                self.buffer = Vec::with_capacity(0x10000);
            }

            Some((counter, new))
        }
    }
}
