// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "kern", no_std, no_main, feature(lang_items))]

#[cfg(feature = "kern")]
use ebpf_kern as ebpf;
#[cfg(feature = "user")]
use ebpf_user as ebpf;

use core::mem;
use bpf_recorder::SocketId;

#[cfg(feature = "kern")]
ebpf::license!("GPL");

#[cfg(any(feature = "kern", feature = "user"))]
#[derive(ebpf::BpfApp)]
pub struct App {
    #[ringbuf(size = 0x8000000)]
    pub event_queue: ebpf::RingBufferRef,
    #[hashmap(size = 64)]
    pub ports: ebpf::HashMapRef<2, 4>,
    #[hashmap(size = 64)]
    pub processes: ebpf::HashMapRef<4, 2>,
    #[hashmap(size = 0x2000)]
    pub connections: ebpf::HashMapRef<{mem::size_of::<SocketId>()}, 4>,
    #[hashmap(size = 0x100)]
    pub syscall_contexts: ebpf::HashMapRef<4, 0x20>,
    #[prog("tracepoint/syscalls/sys_enter_bind")]
    pub enter_bind: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_bind")]
    pub exit_bind: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_connect")]
    pub enter_connect: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_connect")]
    pub exit_connect: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_accept4")]
    pub enter_accept4: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_accept4")]
    pub exit_accept4: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_close")]
    pub enter_close: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_close")]
    pub exit_close: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_write")]
    pub enter_write: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_write")]
    pub exit_write: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_read")]
    pub enter_read: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_read")]
    pub exit_read: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_sendto")]
    pub enter_sendto: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_sendto")]
    pub exit_sendto: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_recvfrom")]
    pub enter_recvfrom: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_exit_recvfrom")]
    pub exit_recvfrom: ebpf::ProgRef,
}

#[cfg(feature = "kern")]
mod syscall_context;

#[cfg(feature = "kern")]
mod send;

#[cfg(feature = "kern")]
mod address;

#[cfg(feature = "kern")]
use {
    core::ptr,
    ebpf::helpers,
    bpf_recorder::{EventId, DataTag},
    self::syscall_context::{SyscallContext, SyscallContextData},
    self::address::Address,
};

#[cfg(feature = "kern")]
impl App {
    #[inline(always)]
    fn push(&mut self, thread_id: u32, ts: u64, data: SyscallContextData) -> Result<(), i32> {
        let mut context = SyscallContext {
            data: SyscallContextData::Empty,
            ts,
        };
        // bpf validator forbids reading from stack uninitialized data
        // different variants of this enum has different length,
        unsafe { ptr::write_volatile(&mut context.data, mem::zeroed()) };
        context.data = data;

        self.syscall_contexts.insert_unsafe(thread_id.to_ne_bytes(), context)
    }

    #[inline(always)]
    fn pop(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        let (pid, thread_id) = {
            let x = unsafe { helpers::get_current_pid_tgid() };
            ((x >> 32) as u32, (x & 0xffffffff) as u32)
        };
        let ts1 = unsafe { helpers::ktime_get_ns() };

        match self.syscall_contexts.remove_unsafe::<SyscallContext>(&thread_id.to_ne_bytes())? {
            Some(context) => {
                let SyscallContext { data, ts: ts0 } = context;
                let ret = ctx.read_here(0x10);
                self.on_ret(ret, data, ts0, ts1, pid)
            },
            None => Err(-1),
        }
    }

    #[inline(always)]
    fn is_connected(&self, socket_id: SocketId) -> bool {
        if let Some(c) = self.connections.get(&socket_id.to_ne_bytes()) {
            let c = u32::from_ne_bytes(*c);
            c == 1 || c == 2
        } else {
            false
        }
    }

    #[inline(always)]
    fn is_process(&self, pid: u32) -> bool {
        self.processes.get(&pid.to_ne_bytes()).is_some()
    }

    fn is_interesting_port(&self, port: u16) -> bool {
        self.ports.get(&port.to_ne_bytes()).is_some()
    }

    fn reg_process(&mut self, pid: u32, port: u16) -> Result<(), i32> {
        self.processes.insert(pid.to_ne_bytes(), port.to_ne_bytes())
    }

    fn reg_connection(&mut self, socket_id: SocketId, incoming: bool) -> Result<(), i32> {
        let _ = self.forget_connection(socket_id);
        let v = if incoming { 2u32 } else { 1u32 };
        self.connections.insert(socket_id.to_ne_bytes(), v.to_ne_bytes())
    }

    fn forget_connection(&mut self, socket_id: SocketId) -> Result<(), i32> {
        self.connections.remove(&socket_id.to_ne_bytes())?;
        Ok(())
    }

    #[inline(always)]
    fn on_data(&mut self, ctx: ebpf::Context, incoming: bool, net: bool) -> Result<(), i32> {
        let fd = ctx.read_here::<u64>(0x10) as u32;
        let data_ptr = ctx.read_here::<u64>(0x18);
        let (pid, thread_id) = {
            let x = unsafe { helpers::get_current_pid_tgid() };
            ((x >> 32) as u32, (x & 0xffffffff) as u32)
        };
        if !self.is_connected(SocketId { pid, fd }) {
            return Ok(());
        }

        let ts = unsafe { helpers::ktime_get_ns() };
        let data = match (incoming, net) {
            (false, false) => SyscallContextData::Write { fd, data_ptr },
            (false, true) => SyscallContextData::Send { fd, data_ptr },
            (true, false) => SyscallContextData::Read { fd, data_ptr },
            (true, true) => SyscallContextData::Recv { fd, data_ptr },
        };

        self.push(thread_id, ts, data)
    }

    #[inline(always)]
    fn on_connection(&mut self, ctx: ebpf::Context, incoming: bool) -> Result<(), i32> {
        let fd = ctx.read_here::<u64>(0x10) as u32;
        let addr_ptr = ctx.read_here::<u64>(0x18);
        let addr_len = ctx.read_here::<u64>(0x20);
        let (pid, thread_id) = {
            let x = unsafe { helpers::get_current_pid_tgid() };
            ((x >> 32) as u32, (x & 0xffffffff) as u32)
        };
        let ts = unsafe { helpers::ktime_get_ns() };

        if !self.is_process(pid) {
            return Ok(());
        }

        let data = if incoming {
            SyscallContextData::Accept { listen_on_fd: fd, addr_ptr, addr_len }
        } else {
            SyscallContextData::Connect { fd, addr_ptr, addr_len }
        };

        self.push(thread_id, ts, data)
    }

    #[inline(always)]
    fn on_ret(
        &mut self,
        ret: i64,
        data: SyscallContextData,
        ts0: u64,
        ts1: u64,
        pid: u32,
    ) -> Result<(), i32> {
        if ret < 0 {
            // TODO: need a better fix
            // EINPROGRESS
            //     The socket is nonblocking and the connection cannot be
            //     completed immediately.  (UNIX domain sockets failed with
            //     EAGAIN instead.)  It is possible to select(2) or poll(2)
            //     for completion by selecting the socket for writing.  After
            //     select(2) indicates writability, use getsockopt(2) to read
            //     the SO_ERROR option at level SOL_SOCKET to determine
            //     whether connect() completed successfully (SO_ERROR is
            //     zero) or unsuccessfully (SO_ERROR is one of the usual
            //     error codes listed here, explaining the reason for the
            //     failure).
            const EINPROGRESS: i64 = -115;
            if !(matches!(&data, &SyscallContextData::Connect { .. }) && ret == EINPROGRESS) {
                return Ok(());
            }
        }

        match data {
            SyscallContextData::Empty => Ok(()),
            SyscallContextData::Bind { fd, addr_ptr, addr_len } => {
                let address = Address::read(addr_ptr, addr_len)?.ok_or(-1)?;
                let port = address.port();
                if !self.is_interesting_port(port) {
                    return Ok(());
                }
                self.reg_process(pid, port)?;

                let id = EventId::new(SocketId { pid, fd }, ts0, ts1);
                send::sized::<typenum::U28, typenum::B0>(
                    id,
                    DataTag::Bind,
                    addr_ptr as *mut u8,
                    addr_len as usize,
                    &mut self.event_queue,
                );
                Ok(())
            },
            SyscallContextData::Connect { fd, addr_ptr, addr_len } => {
                let socket_id = SocketId { pid, fd };
                if Address::read(addr_ptr, addr_len)?.is_none() {
                    return self.forget_connection(socket_id);
                }
                self.reg_connection(socket_id, false)?;
                let id = EventId::new(socket_id, ts0, ts1);
                send::sized::<typenum::U28, typenum::B0>(
                    id,
                    DataTag::Connect,
                    addr_ptr as *const u8,
                    addr_len as usize,
                    &mut self.event_queue,
                );
                Ok(())
            },
            SyscallContextData::Accept { listen_on_fd, addr_ptr, addr_len } => {
                let _ = listen_on_fd;
                let fd = ret as u32;
                let socket_id = SocketId { pid, fd };
                if Address::read(addr_ptr, addr_len)?.is_none() {
                    return self.forget_connection(socket_id);
                }
                self.reg_connection(socket_id, true)?;
                let id = EventId::new(socket_id, ts0, ts1);
                send::sized::<typenum::U28, typenum::B0>(
                    id,
                    DataTag::Accept,
                    addr_ptr as *const u8,
                    addr_len as usize,
                    &mut self.event_queue,
                );
                Ok(())
            },
            SyscallContextData::Write { fd, data_ptr }
            | SyscallContextData::Send { fd, data_ptr }
            | SyscallContextData::Read { fd, data_ptr }
            | SyscallContextData::Recv { fd, data_ptr } => {
                let id = EventId::new(SocketId { pid, fd }, ts0, ts1);
                send::dyn_sized::<typenum::B0>(
                    id,
                    data.tag(),
                    data_ptr as *mut u8,
                    ret as usize,
                    &mut self.event_queue,
                );
                Ok(())
            },
        }
    }

    #[inline(always)]
    pub fn enter_bind(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        let fd = ctx.read_here::<u64>(0x10) as u32;
        let addr_ptr = ctx.read_here::<u64>(0x18);
        let addr_len = ctx.read_here::<u64>(0x20);
        let (_, thread_id) = {
            let x = unsafe { helpers::get_current_pid_tgid() };
            ((x >> 32) as u32, (x & 0xffffffff) as u32)
        };
        let ts = unsafe { helpers::ktime_get_ns() };

        let data = SyscallContextData::Bind { fd, addr_ptr, addr_len };

        self.push(thread_id, ts, data)
    }

    #[inline(always)]
    pub fn exit_bind(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_connect(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_connection(ctx, false)
    }

    #[inline(always)]
    pub fn exit_connect(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_accept4(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_connection(ctx, true)
    }

    #[inline(always)]
    pub fn exit_accept4(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_close(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        let fd = ctx.read_here::<u64>(0x10) as u32;
        let (pid, _) = {
            let x = unsafe { helpers::get_current_pid_tgid() };
            ((x >> 32) as u32, (x & 0xffffffff) as u32)
        };
        let ts = unsafe { helpers::ktime_get_ns() };
        let socket_id = SocketId { pid, fd };

        if !self.is_process(pid) {
            return Ok(());
        }
        if !self.is_connected(socket_id) {
            return Ok(());
        }

        self.connections.remove(&socket_id.to_ne_bytes())?;
        let id = EventId::new(SocketId { pid, fd }, ts, ts);
        send::sized::<typenum::U0, typenum::B0>(id, DataTag::Close, ptr::null(), 0, &mut self.event_queue);

        Ok(())
    }

    #[inline(always)]
    pub fn exit_close(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        let _ = ctx;
        Ok(())
    }

    #[inline(always)]
    pub fn enter_write(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_data(ctx, false, false)
    }

    #[inline(always)]
    pub fn exit_write(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_sendto(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_data(ctx, false, true)
    }

    #[inline(always)]
    pub fn exit_sendto(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_read(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_data(ctx, true, false)
    }

    #[inline(always)]
    pub fn exit_read(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }

    #[inline(always)]
    pub fn enter_recvfrom(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.on_data(ctx, true, true)
    }

    #[inline(always)]
    pub fn exit_recvfrom(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.pop(ctx)
    }
}

#[cfg(feature = "user")]
fn main() {
    use ebpf::{Skeleton, kind::{AppItemKindMut, AppItem}};
    use std::{
        fs,
        io::{Error, BufReader, BufRead},
        os::unix::{fs::PermissionsExt, net::UnixListener},
        process,
        str::FromStr,
    };
    use bpf_recorder::Command;
    use tracing::Level;
    use passfd::FdPassingExt;

    sudo::escalate_if_needed().expect("failed to obtain superuser permission");
    ctrlc::set_handler(move || process::exit(0)).expect("failed to setup ctrl+c handler");
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let socket = "/tmp/bpf-sniffer.sock";
    let _ = fs::remove_file(socket);
    let _ = fs::create_dir("/tmp");
    let listener = UnixListener::bind(socket).expect("failed to bind socket");

    let mut perms = fs::metadata(socket)
        .expect("failed to get permission of socket")
        .permissions();
    perms.set_mode(0o666);
    fs::set_permissions(socket, perms).expect("failed to set permission for socket");

    static CODE: &[u8] = include_bytes!(concat!("../", env!("BPF_CODE_RECORDER")));

    let mut skeleton = Skeleton::<App>::open("bpf-recorder\0", CODE)
        .unwrap_or_else(|code| panic!("failed to open bpf: {}", code));
    skeleton.load()
        .unwrap_or_else(|code| panic!("failed to load bpf: {}", code));
    skeleton.attach()
        .unwrap_or_else(|code| panic!("failed to attach bpf: {}", code));
    log::info!("attached bpf module");

    let fd = match skeleton.app.event_queue.kind_mut() {
        AppItemKindMut::Map(map) => map.fd(),
        _ => unreachable!(),
    };

    let (stream, address) = listener.accept().expect("failed to accept connection");
    log::info!("accept client: {:?}", address);

    stream
        .send_fd(fd)
        .expect("failed to send ring buffer access");

    let stream = BufReader::new(stream);
    for line in stream.lines() {
        // handle line
        match line {
            Ok(line) => match { log::info!("command: {}", line); Command::from_str(&line) } {
                Ok(Command::FetchCounter) => (),
                Ok(Command::WatchPort { port }) => {
                    match skeleton.app.ports.insert(port.to_ne_bytes(), 1u32.to_ne_bytes()) {
                        Ok(()) => (),
                        Err(code) => {
                            tracing::error!(
                                "failed to watch port {}, code {}, error {}",
                                port,
                                code,
                                Error::last_os_error(),
                            );
                        },
                    }
                },
                Ok(Command::IgnoreConnection { pid, fd }) => {
                    let socket_id = SocketId { pid, fd };
                    match skeleton.app.connections.remove(&socket_id.to_ne_bytes()) {
                        Ok(()) => (),
                        Err(code) => {
                            tracing::error!(
                                "failed to ignore connection {}, code {}, error {}",
                                socket_id,
                                code,
                                Error::last_os_error(),
                            );
                        },
                    }
                },
                Err(error) => {
                    tracing::warn!("bad command: {}", error);
                },
            },
            Err(error) => tracing::warn!("failed to read command: {}", error),
        }
    }

    log::info!("detached bpf module");
}
