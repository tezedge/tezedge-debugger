// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::{TryInto, TryFrom}, io::{self, Read, Write}};

use tezos_messages::p2p::binary_message::{BinaryRead, BinaryMessage, BinaryChunk};
use crypto::{crypto_box::PrecomputedKey, nonce::Nonce};

pub trait Message
where
    Self: Sized,
{
    fn read_msg(
        stream: &mut impl Read,
        buffer: &mut ChunkBuffer,
        key: &PrecomputedKey,
        nonce: Nonce,
        // whether peer message, or meta/ack
        peer_message: bool,
    ) -> io::Result<(Nonce, Self)>;

    fn write_msg(
        &self,
        stream: &mut impl Write,
        key: &PrecomputedKey,
        nonce: Nonce,
    ) -> Nonce;
}

impl<M> Message for M
where
    M: BinaryMessage,
{
    fn write_msg(
        &self,
        stream: &mut impl Write,
        key: &PrecomputedKey,
        nonce: Nonce,
    ) -> Nonce {
        let bytes = self.as_bytes().unwrap();
        let mut nonce = nonce;
        for bytes in bytes.as_slice().chunks(0xffe0) {
            let temp = key.encrypt(&bytes, &nonce).unwrap();
            let chunk = BinaryChunk::from_content(&temp).unwrap().raw().clone();
            stream.write_all(&chunk).unwrap();
            nonce = nonce.increment();
        }

        nonce
    }

    fn read_msg(
        stream: &mut impl Read,
        buffer: &mut ChunkBuffer,
        key: &PrecomputedKey,
        nonce: Nonce,
        peer_message: bool,
    ) -> io::Result<(Nonce, Self)>
    where
        M: BinaryRead,
    {
        const HEADER_LENGTH: usize = 4;

        let mut nonce = nonce;
        let mut bytes = Vec::new();
        let mut length = 0;
        loop {
            let chunk = buffer.read_chunk(stream)?;
            bytes.extend_from_slice(&key.decrypt(chunk.content(), &nonce).unwrap());
            if length == 0 && peer_message {
                let b = TryFrom::try_from(&bytes[..HEADER_LENGTH]).unwrap();
                length = u32::from_be_bytes(b) as usize + HEADER_LENGTH;
            }
            nonce = nonce.increment();

            if bytes.len() == length || !peer_message {
                break Ok((nonce, M::from_bytes(bytes).unwrap()));
            }
        }
    }
}

pub struct ChunkBuffer {
    len: usize,
    data: [u8; 0x10000],
}

impl Default for ChunkBuffer {
    fn default() -> Self {
        ChunkBuffer {
            len: 0,
            data: [0; 0x10000],
        }
    }
}

impl ChunkBuffer {
    pub fn read_chunk(&mut self, stream: &mut impl Read) -> io::Result<BinaryChunk> {
        const HEADER_LENGTH: usize = 2;
        loop {
            if self.len >= HEADER_LENGTH {
                let chunk_len = (self.data[0] as usize) * 256 + (self.data[1] as usize);
                let raw_len = chunk_len + HEADER_LENGTH;
                if self.len >= raw_len {
                    let chunk = self.data[..(raw_len)].to_vec();
                    for i in raw_len..self.len {
                        self.data[(i - raw_len)] = self.data[i];
                    }
                    self.len -= raw_len;
                    return Ok(chunk.try_into().unwrap());
                }
            }
            let read = stream.read(&mut self.data[self.len..])?;
            self.len += read;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
