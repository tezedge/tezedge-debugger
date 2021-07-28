// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::TcpStream;

use crypto::{
    nonce::{NoncePair, Nonce, generate_nonces},
    crypto_box::{CryptoKey, PrecomputedKey, PublicKey, SecretKey},
};
use tezos_messages::p2p::{
    binary_message::{BinaryChunk, BinaryRead, BinaryWrite},
    encoding::{
        connection::ConnectionMessage,
        version::NetworkVersion,
    },
};
use super::buffer::ChunkBuffer;

fn identity(json: &str, port: u16) -> (ConnectionMessage, SecretKey) {
    use tezos_identity::Identity;

    let identity = Identity::from_json(&json).unwrap();
    let version = NetworkVersion::new("TEZOS_MAINNET".to_string(), 0, 1);
    let connection_message = ConnectionMessage::try_new(
        port,
        &identity.public_key,
        &identity.proof_of_work_stamp,
        Nonce::random(),
        version,
    ).unwrap();

    (connection_message, identity.secret_key)
}

pub fn initiator(this: u16, stream: &mut TcpStream) -> (PrecomputedKey, NoncePair) {
    use std::io::Write;
    
    let (connection_message, sk) = identity(include_str!("../identity_i.json"), this);

    let temp = connection_message.as_bytes().unwrap();
    let initiator_chunk = BinaryChunk::from_content(&temp).unwrap();
    stream.write_all(initiator_chunk.raw()).unwrap();
    let responder_chunk = ChunkBuffer::default().read_chunk(stream).unwrap();

    let connection_message = ConnectionMessage::from_bytes(responder_chunk.content()).unwrap();
    let pk = PublicKey::from_bytes(connection_message.public_key()).unwrap();

    let key = PrecomputedKey::precompute(&pk, &sk);
    let pair = generate_nonces(initiator_chunk.raw(), &responder_chunk.raw(), false).unwrap();
    (key, pair)
}

pub fn responder(this: u16, stream: &mut TcpStream) -> (PrecomputedKey, NoncePair) {
    use std::io::Write;

    let initiator_chunk = ChunkBuffer::default().read_chunk(stream).unwrap();
    let connection_message = ConnectionMessage::from_bytes(initiator_chunk.content()).unwrap();
    let pk = PublicKey::from_bytes(connection_message.public_key()).unwrap();
    let (connection_message, sk) = identity(include_str!("../identity_r.json"), this);
    let temp = connection_message.as_bytes().unwrap();
    let responder_chunk = BinaryChunk::from_content(&temp).unwrap();
    stream.write_all(responder_chunk.raw()).unwrap();

    let key = PrecomputedKey::precompute(&pk, &sk);
    let pair = generate_nonces(responder_chunk.raw(), initiator_chunk.raw(), true).unwrap();
    (key, pair)
}
