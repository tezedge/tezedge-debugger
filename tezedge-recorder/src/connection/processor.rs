// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{net::SocketAddr, sync::Arc};
use anyhow::Result;
use thiserror::Error;
use either::Either;
use crypto::{CryptoError, proof_of_work};
use super::{key::Key, chunk_buffer::Buffer, Identity, Database, tables::connection};

pub struct Connection<Db> {
    address: SocketAddr,
    incoming: bool,
    handshake: Either<Identity, Result<Key, CryptoError>>,
    input_state: Buffer,
    output_state: Buffer,
    db: Arc<Db>,
}

#[derive(Error, Debug)]
pub enum HandshakeWarning {
    #[error("Connection message is too short {}", _0)]
    ConnectionMessageTooShort(usize),
    #[error("Proof-of-work check failed")]
    PowInvalid(f64),
}

impl<Db> Connection<Db>
where
    Db: Database,
{
    pub fn new(
        address: SocketAddr,
        incoming: bool,
        identity: Identity,
        db: Arc<Db>,
    ) -> Self {
        Connection {
            address,
            incoming,
            handshake: Either::Left(identity),
            input_state: Buffer::default(),
            output_state: Buffer::default(),
            db,
        }
    }

    #[allow(dead_code)]
    fn store_connection(&self, peer_id: Option<String>) {
        let it = connection::Item::new(self.incoming, self.address.clone(), peer_id);
        self.db.store_connection(it);
    }

    fn buffer_mut(&mut self, incoming: bool) -> &mut Buffer {
        if incoming {
            &mut self.input_state
        } else {
            &mut self.output_state
        }
    }

    pub fn handle_data(&mut self, payload: &[u8], incoming: bool) -> Result<()> {
        self.buffer_mut(incoming).handle_data(&payload);

        let check = |payload: &[u8]| -> Result<(), HandshakeWarning> {
            if payload.len() <= 88 {
                return Err(HandshakeWarning::ConnectionMessageTooShort(payload.len()));
            }
            // TODO: move to config
            let target = 26.0;
            if proof_of_work::check_proof_of_work(&payload[4..60], target).is_err() {
                return Err(HandshakeWarning::PowInvalid(target));
            }

            Ok(())
        };

        // TODO: handle error, write in database
        match &self.handshake {
            Either::Left(identity) => {
                if self.input_state.have_chunk() && self.output_state.have_chunk() {
                    let incoming_chunk = self.input_state.next().unwrap();
                    if let Err(_warning) = check(&incoming_chunk) {
                        // handle this
                    }
                    let outgoing_chunk = self.output_state.next().unwrap();
                    if let Err(_warning) = check(&outgoing_chunk) {
                        // handle this
                    }
                    let (initiator, responder) = if self.incoming {
                        (incoming_chunk, outgoing_chunk)
                    } else {
                        (outgoing_chunk, incoming_chunk)
                    };
                    let key = Key::new(identity, &initiator, &responder);
                    // write db
                    log::info!("have a connection");
                    self.handshake = Either::Right(key);
                }
            },
            Either::Right(Ok(key)) => {
                let mut key = key.clone();
                for payload in self.buffer_mut(incoming) {
                    let _decrypted = key.decrypt(&payload, incoming).unwrap_or(Vec::new());
                    // write db
                    log::info!("have a chunk");
                }
                self.handshake = Either::Right(Ok(key));
            },
            Either::Right(Err(_error)) => {
                // write db
            },
        };

        Ok(())
    }

    pub fn join(self) {
    }
}
