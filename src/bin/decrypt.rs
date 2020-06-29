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
    let sent = [0, 134, 38, 4, 126, 51, 0, 70, 86, 5, 52, 207, 155, 13, 224, 153, 108, 159, 134, 135, 184, 17, 84, 180, 180, 55, 172, 199, 62, 127, 208, 207, 202, 51, 246, 31, 46, 20, 176, 37, 207, 53, 81, 153, 169, 76, 229, 108, 47, 125, 16, 152, 114, 87, 244, 30, 86, 225, 1, 129, 204, 108, 109, 51, 33, 87, 225, 145, 66, 252, 118, 95, 215, 254, 10, 17, 163, 167, 30, 31, 204, 228, 169, 202, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1].to_vec();
    let recv = [0, 134, 38, 4, 22, 138, 26, 190, 155, 136, 40, 188, 104, 21, 142, 120, 74, 112, 73, 226, 28, 188, 210, 36, 182, 108, 243, 143, 14, 181, 24, 46, 142, 252, 160, 93, 167, 156, 211, 24, 253, 18, 59, 165, 169, 245, 0, 23, 37, 19, 76, 248, 81, 96, 25, 99, 250, 192, 179, 235, 179, 110, 231, 221, 123, 122, 182, 154, 109, 125, 161, 34, 16, 226, 86, 165, 206, 218, 206, 207, 78, 191, 106, 68, 0, 0, 0, 44, 84, 69, 90, 79, 83, 95, 65, 76, 80, 72, 65, 78, 69, 84, 95, 67, 65, 82, 84, 72, 65, 71, 69, 95, 50, 48, 49, 57, 45, 49, 49, 45, 50, 56, 84, 49, 51, 58, 48, 50, 58, 49, 51, 90, 0, 0, 0, 1].to_vec();
    let sk = "6145bed5ec82eea37d1ec7a2b8a85d2eb36306ed36f5144ff144344de72ead69";
    let sent_data = BinaryChunk::try_from(sent.clone())
        .unwrap();
    let recv_data = BinaryChunk::try_from(recv.clone())
        .unwrap();

    let sent_cm = ConnectionMessage::try_from(BinaryChunk::try_from(sent.clone())
        .unwrap()).unwrap();
    let recv_cm = ConnectionMessage::try_from(BinaryChunk::try_from(recv.clone())
        .unwrap()).unwrap();

    println!("sent: {:?}", hex::encode(&sent_cm.public_key));
    println!("recv: {:?}", hex::encode(&recv_cm.public_key));

    let NoncePair { local, remote } = generate_nonces(
        sent_data.raw(),
        recv_data.raw(),
        false,
    );
    // remote=Nonce { value: BigUint { data: [2186764709, 1115102113, 1926412243, 3219579277, 331240414, 2413948809] } }
    // local=Nonce { value: BigUint { data: [1411995813, 3019352145, 2106126191, 2213770376, 2463881867, 683386874] } }
    println!("{:?}, {:?}", remote, local);

    let pk = precompute(
        &hex::encode(&recv_cm.public_key),
        sk,
    ).unwrap();

    // let metadata = [0, 18, 153, 164, 248, 149, 28, 25, 155, 121, 5, 130, 116, 104, 99, 33, 166, 161, 58, 32].to_vec();
    // let metadata = [0, 18, 96, 240, 74, 254, 86, 143, 110, 101, 94, 222, 199, 40, 67, 113, 117, 156, 194, 142].to_vec();
    println!("{:?}", decrypt(&[80, 160, 180, 27, 249, 122, 101, 251, 221, 217, 53, 240, 69, 17, 163, 223, 43, 43], &local, &pk));
    println!("{:?}", decrypt(&[80, 160, 180, 27, 249, 122, 101, 251, 221, 217, 53, 240, 69, 17, 163, 223, 43, 43], &remote, &pk));
}