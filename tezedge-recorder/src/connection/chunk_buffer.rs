// Copyright (c) SimpleStaking and Tezedge Contributors
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

    #[allow(dead_code)]
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
        let len = self.len()? + 2;
        if self.buffer.len() < len {
            None
        } else {
            let counter = self.counter;
            self.counter += 1;
            //let remaining = self.buffer.split_off(len);
            //Some((counter, mem::replace(&mut self.buffer, remaining)))

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
