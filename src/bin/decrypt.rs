// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::BinaryChunk;
use std::convert::TryFrom;
use crypto::nonce::{generate_nonces, NoncePair};
use crypto::crypto_box::{precompute, decrypt};
use tezos_messages::p2p::encoding::prelude::ConnectionMessage;

// {
//      "peer_id":"idsQqW5E9mH3SNNd56uUnALb8RxvdM",
//      "public_key":"7e330046560534cf9b0de0996c9f8687b81154b4b437acc73e7fd0cfca33f61f",
//      "secret_key":"6145bed5ec82eea37d1ec7a2b8a85d2eb36306ed36f5144ff144344de72ead69",
//      "proof_of_work_stamp":"2e14b025cf355199a94ce56c2f7d10987257f41e56e10181"
// }

pub fn main() {
    let sent = [0, 134, 38, 4, 218, 225, 35, 230, 150, 102, 12, 177, 153, 161, 78, 250, 70, 108, 224, 94, 55, 112, 49, 151, 152, 109, 59, 99, 28, 76, 95, 96, 95, 37, 145, 17, 141, 153, 255, 91, 182, 221, 192, 61, 210, 86, 81, 88, 10, 19, 0, 120, 193, 110, 238, 194, 37, 57, 190, 149, 64, 109, 67, 172, 228, 155, 207, 182, 44, 111, 86, 207, 101, 3, 25, 90, 92, 31, 175, 68, 234, 85, 155, 75, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1] .to_vec();
    let recv = [0, 134, 38, 4, 22, 138, 26, 190, 155, 136, 40, 188, 104, 21, 142, 120, 74, 112, 73, 226, 28, 188, 210, 36, 182, 108, 243, 143, 14, 181, 24, 46, 142, 252, 160, 93, 167, 156, 211, 24, 253, 18, 59, 165, 169, 245, 0, 23, 37, 19, 76, 248, 81, 96, 25, 99, 250, 192, 179, 235, 97, 37, 139, 57, 240, 151, 166, 90, 177, 37, 153, 220, 204, 159, 30, 120, 115, 79, 213, 3, 149, 108, 128, 59, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1].to_vec();
    let sk = "69b90b12d2581d6b13c481a0b48675a8a81f3f0390918519a0e83f077a3baf25";
    let sent_data = BinaryChunk::try_from(sent.clone())
        .unwrap();
    let recv_data = BinaryChunk::try_from(recv.clone())
        .unwrap();

    // let sent_cm = ConnectionMessage::try_from(BinaryChunk::try_from(sent.clone())
    //     .unwrap()).unwrap();
    let recv_cm = ConnectionMessage::try_from(BinaryChunk::try_from(recv.clone())
        .unwrap()).unwrap();

    let NoncePair { local, remote } = generate_nonces(
        sent_data.raw(),
        recv_data.raw(),
        false,
    );
    println!("remote: {:?}\nlocal: {:?}", remote, local);

    let pk = precompute(
        &hex::encode(&recv_cm.public_key),
        sk,
    ).unwrap();

    println!("pk: {}", hex::encode(pk.as_ref().as_ref()));

    println!("{:?}", decrypt(&[50, 127, 56, 196, 22, 227, 36, 219, 15, 203, 176, 2, 113, 70, 146, 187, 8, 65], &local, &pk));
    println!("{:?}", decrypt(&[50, 127, 56, 196, 22, 227, 36, 219, 15, 203, 176, 2, 113, 70, 146, 187, 8, 65], &remote, &pk));
}