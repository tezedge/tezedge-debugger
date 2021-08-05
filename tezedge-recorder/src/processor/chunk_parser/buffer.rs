// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub struct Buffer {
    counter: u64,
    buffer: Vec<u8>,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            counter: 0,
            buffer: Vec::with_capacity(0x10000),
        }
    }
}

impl Buffer {
    pub fn handle_data(&mut self, payload: &[u8]) {
        if self.have_chunk().is_some() {
            log::debug!(
                "append new data while not consumed chunk, buffer len: {}, counter: {}",
                self.buffer.len(),
                self.counter,
            );
        }
        self.buffer.extend_from_slice(payload);
    }

    pub fn remaining(&self) -> usize {
        self.buffer.len()
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }

    // length (including 2 bytes header) of the chunk at offset
    fn len(&self, offset: usize) -> Option<usize> {
        use std::convert::TryFrom;

        if self.buffer.len() >= offset + 2 {
            let b = <[u8; 2]>::try_from(&self.buffer[offset..(offset + 2)]).unwrap();
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

    pub fn cleanup(&mut self) -> Option<(u64, Vec<u8>)> {
        use std::mem;

        if self.buffer.is_empty() {
            return None;
        }

        let counter = self.counter;
        self.counter += 1;
        Some((counter, mem::replace(&mut self.buffer, Vec::new())))
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
