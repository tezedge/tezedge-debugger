// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub struct Buffer {
    buffer: Vec<u8>,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer { buffer: Vec::with_capacity(0x10000) }
    }
}

impl Buffer {
    pub fn handle_data(&mut self, payload: &[u8]) {
        self.buffer.extend_from_slice(payload);
    }

    fn len(&self) -> Option<usize> {
        if self.buffer.len() < 2 {
            return None;
        }
        Some((self.buffer[0] as usize) * 256 + (self.buffer[1] as usize))
    }

    pub fn have_chunk(&self) -> bool {
        self.buffer.len() >= 2 + self.len().unwrap_or(0)
    }
}

impl Iterator for Buffer {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::mem;

        let len = self.len()?;
        if self.buffer.len() < 2 + len {
            None
        } else if self.buffer.len() == 2 + len {
            Some(mem::replace(&mut self.buffer, Vec::new()))
        } else {
            let remaining = self.buffer.split_off(2 + len);
            Some(mem::replace(&mut self.buffer, remaining))
        }
    }
}
