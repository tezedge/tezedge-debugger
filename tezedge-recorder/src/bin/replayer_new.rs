// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    net::{TcpListener, SocketAddr},
    time::Duration,
    io::{Read, Write},
};
use tezedge_recorder::database::{
    DatabaseNew,
    rocks::Db,
    rocks_utils::{SyscallKind, SyscallMetadata},
};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let db = Db::open("/volume/debugger_db/tezedge", false, None, None).unwrap();
    let it = db.syscall_metadata_iterator(0).unwrap();

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 9732))).unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(1_000)))
        .unwrap();

    for item in it {
        let SyscallMetadata { inner, .. } = item.unwrap();
        log::info!("{:?}", inner);
        match inner {
            SyscallKind::Read(Ok(data)) => {
                stream.write_all(&data).unwrap();
            },
            SyscallKind::Write(Ok(data)) => {
                let mut buffer = vec![0; data.len()];
                let _ = stream.read(&mut buffer).unwrap();
                assert_eq!(buffer, data);
            },
            _ => (),
        }
    }
}
