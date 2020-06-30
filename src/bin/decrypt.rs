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
    let sent = [0, 134, 38, 4, 147, 30, 238, 193, 214, 132, 173, 249, 16, 111, 69, 201, 18, 251, 144, 176, 50, 115, 225, 179, 179, 180, 48, 35, 152, 6, 52, 240, 207, 27, 25, 77, 158, 157, 116, 242, 86, 0, 191, 197, 193, 27, 250, 119, 143, 164, 115, 20, 154, 249, 183, 79, 122, 26, 196, 132, 107, 47, 117, 239, 129, 251, 143, 89, 181, 1, 99, 194, 97, 0, 88, 90, 241, 149, 222, 50, 39, 119, 147, 73, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1].to_vec();
    let recv = [0, 134, 38, 4, 22, 138, 26, 190, 155, 136, 40, 188, 104, 21, 142, 120, 74, 112, 73, 226, 28, 188, 210, 36, 182, 108, 243, 143, 14, 181, 24, 46, 142, 252, 160, 93, 167, 156, 211, 24, 253, 18, 59, 165, 169, 245, 0, 23, 37, 19, 76, 248, 81, 96, 25, 99, 250, 192, 179, 235, 77, 22, 25, 250, 160, 29, 125, 115, 227, 11, 9, 134, 93, 62, 151, 195, 129, 145, 105, 146, 223, 62, 120, 209, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1].to_vec();
    let sk = "5c0a8c7cd940213bda75c8b28caeabeba84d063c63f673529eb53408b03c5a89";
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

    println!("{:?}", decrypt(&[8, 147, 184, 67, 216, 173, 28, 9, 83, 25, 155, 186, 122, 51, 145, 143, 56, 195], &local, &pk));
    println!("{:?}", decrypt(&[8, 147, 184, 67, 216, 173, 28, 9, 83, 25, 155, 186, 122, 51, 145, 143, 56, 195], &remote, &pk));
}