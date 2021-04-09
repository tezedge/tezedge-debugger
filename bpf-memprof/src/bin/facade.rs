// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "facade")]

use std::{
    fs,
    io::{BufReader, BufRead},
    os::unix::{fs::PermissionsExt, io::AsRawFd, net::UnixListener},
    process,
};
use tracing::Level;
use structopt::StructOpt;
use passfd::FdPassingExt;

use redbpf::load::Loader;

#[derive(StructOpt)]
pub struct Opts {
    #[structopt(
        short,
        long,
        default_value = "/tmp/bpf-memprof.sock",
        help = "communication channel"
    )]
    socket: String,
}

pub fn main() {
    let Opts { socket } = StructOpt::from_args();

    sudo::escalate_if_needed().expect("failed to obtain superuser permission");
    ctrlc::set_handler(move || process::exit(0)).expect("failed to setup ctrl+c handler");

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).expect("failed to bind socket");

    let mut perms = fs::metadata(&socket)
        .expect("failed to get permission of socket")
        .permissions();
    perms.set_mode(0o666);
    fs::set_permissions(&socket, perms).expect("failed to set permission for socket");

    let (stream, address) = listener.accept().expect("failed to accept connection");
    tracing::info!("accept client: {:?}", address);

    let mut loaded = Loader::load(CODE).expect("error loading BPF program");
    for probe in loaded.kprobes_mut() {
        // try to detach the kprobe, if previous run of the sniffer did not cleanup
        let _ = probe
            .detach_kprobe_namespace("default", &probe.name());
        probe
            .attach_kprobe_namespace("default", &probe.name(), 0)
            .expect(&format!("error attaching kprobe program {}", probe.name()));
    }

    tracing::info!("load bpf module");

    let rb = loaded
        .module
        .maps
        .iter()
        .find(|m| m.name == "main_buffer")
        .expect("probes should contain `main_buffer` ring buffer");

    stream
        .send_fd(rb.as_raw_fd())
        .expect("failed to send ring buffer access");

    let stream = BufReader::new(stream);
    for line in stream.lines() {
        // handle line
        match line {
            Ok(line) => log::info!("received command: {}", line),
            Err(error) => log::error!("{:?}", error),
        }
    }
}

#[repr(C)]
struct AlignedTo<A, B>
where
    B: ?Sized,
{
    _align: [A; 0],
    bytes: B,
}

static CODE: &'static [u8] = {
    static _ALIGNED: &'static AlignedTo<u64, [u8]> = &AlignedTo {
        _align: [],
        bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/target/bpf/programs/kprobe/kprobe.elf")),
    };
    &_ALIGNED.bytes
};
