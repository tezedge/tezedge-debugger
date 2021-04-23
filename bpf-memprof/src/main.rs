// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "kern", no_std, no_main, feature(lang_items))]

#[cfg(feature = "kern")]
use ebpf_kern as ebpf;
#[cfg(feature = "user")]
use ebpf_user as ebpf;

#[cfg(feature = "kern")]
ebpf::license!("GPL");

#[cfg(any(feature = "kern", feature = "user"))]
#[derive(ebpf::BpfApp)]
pub struct App {
    #[ringbuf(size = 0x40000000)]
    pub event_queue: ebpf::RingBufferRef,
    #[prog("tracepoint/kmem/kfree")]
    pub kfree: ebpf::ProgRef,
    #[prog("tracepoint/kmem/kmalloc")]
    pub kmalloc: ebpf::ProgRef,
    #[prog("tracepoint/kmem/kmalloc_node")]
    pub kmalloc_node: ebpf::ProgRef,
    #[prog("tracepoint/kmem/kmem_cache_alloc")]
    pub cache_alloc: ebpf::ProgRef,
    #[prog("tracepoint/kmem/kmem_cache_alloc_node")]
    pub cache_alloc_node: ebpf::ProgRef,
    #[prog("tracepoint/kmem/kmem_cache_free")]
    pub cache_free: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_alloc")]
    pub page_alloc: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_alloc_extfrag")]
    pub page_alloc_extfrag: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_alloc_zone_locked")]
    pub page_alloc_zone_locked: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_free")]
    pub page_free: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_free_batched")]
    pub page_free_batched: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_pcpu_drain")]
    pub page_pcpu_drain: ebpf::ProgRef,
    #[prog("tracepoint/kmem/rss_stat")]
    pub rss_stat: ebpf::ProgRef,
    #[prog("tracepoint/exceptions/page_fault_user")]
    pub page_fault_user: ebpf::ProgRef,
}

#[cfg(feature = "kern")]
use {
    bpf_memprof::Pod,
    bpf_memprof::{
        KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc,
        PageAllocExtFrag, PageAllocZoneLocked, PageFree, PageFreeBatched, PagePcpuDrain,
        PageFaultUser, RssStat,
    },
    ebpf::helpers,
};

#[cfg(feature = "kern")]
impl App {
    #[inline(always)]
    fn check_name() -> Result<(), i32> {
        let mut comm = [0; 16];

        let x = unsafe { helpers::get_current_comm(&mut comm as *mut _ as _, 16) };
        let pass = true
            && comm[0] == 'l' as i8
            && comm[1] == 'i' as i8
            && comm[2] == 'g' as i8
            && comm[3] == 'h' as i8
            && comm[4] == 't' as i8
            && comm[5] == '-' as i8
            && comm[6] == 'n' as i8
            && comm[7] == 'o' as i8
            && comm[8] == 'd' as i8
            && comm[9] == 'e' as i8;
        if pass { Ok(()) } else { Err(0) }
    }

    // /sys/kernel/debug/tracing/events/kmem/mm_page_alloc/format

    fn output<T>(&mut self, ctx: ebpf::Context) -> Result<(), i32>
    where
        T: Pod,
    {
        let mut data = self.event_queue.reserve(0x10 + T::SIZE)?;
        ctx.read_into(0, &mut data.as_mut()[0x00..0x10]);
        let x = unsafe { helpers::get_current_pid_tgid() };
        let pid = (x >> 32) as u32;
        data.as_mut()[0x08..0x0c].clone_from_slice(&pid.to_ne_bytes());
        data.as_mut()[0x0c..0x10].clone_from_slice(&T::DISCRIMINANT.unwrap_or(0).to_ne_bytes());
        ctx.read_into(0x08, &mut data.as_mut()[0x10..]);
        data.submit();
        Ok(())
    }

    #[inline(always)]
    pub fn kfree(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<KFree>(ctx)
    }

    #[inline(always)]
    pub fn kmalloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<KMAlloc>(ctx)
    }

    #[inline(always)]
    pub fn kmalloc_node(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<KMAllocNode>(ctx)
    }

    #[inline(always)]
    pub fn cache_alloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<CacheAlloc>(ctx)
    }

    #[inline(always)]
    pub fn cache_alloc_node(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<CacheAllocNode>(ctx)
    }

    #[inline(always)]
    pub fn cache_free(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<CacheFree>(ctx)
    }

    #[inline(always)]
    pub fn page_alloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageAlloc>(ctx)
    }

    #[inline(always)]
    pub fn page_alloc_extfrag(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageAllocExtFrag>(ctx)
    }

    #[inline(always)]
    pub fn page_alloc_zone_locked(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageAllocZoneLocked>(ctx)
    }

    #[inline(always)]
    pub fn page_free(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageFree>(ctx)
    }

    #[inline(always)]
    pub fn page_free_batched(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageFreeBatched>(ctx)
    }

    #[inline(always)]
    pub fn page_pcpu_drain(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PagePcpuDrain>(ctx)
    }

    #[inline(always)]
    pub fn rss_stat(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<RssStat>(ctx)
    }

    #[inline(always)]
    pub fn page_fault_user(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        Self::check_name()?;
        self.output::<PageFaultUser>(ctx)
    }
}

#[cfg(feature = "user")]
fn main() {
    use ebpf::{Skeleton, kind::{AppItemKindMut, AppItem}};
    use std::{
        fs,
        io::{BufReader, BufRead},
        os::unix::{fs::PermissionsExt, net::UnixListener},
        process,
    };
    use tracing::Level;
    use passfd::FdPassingExt;
    
    sudo::escalate_if_needed().expect("failed to obtain superuser permission");
    ctrlc::set_handler(move || process::exit(0)).expect("failed to setup ctrl+c handler");
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let socket = "/tmp/bpf-memprof.sock";
    let _ = fs::remove_file(socket);
    let listener = UnixListener::bind(socket).expect("failed to bind socket");

    let mut perms = fs::metadata(socket)
        .expect("failed to get permission of socket")
        .permissions();
    perms.set_mode(0o666);
    fs::set_permissions(socket, perms).expect("failed to set permission for socket");

    static CODE: &[u8] = include_bytes!(concat!("../", env!("BPF_CODE")));

    let mut skeleton = Skeleton::<App>::open("bpf-memprof\0", CODE)
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
            Ok(line) => log::info!("received command: {}", line),
            Err(error) => log::error!("{:?}", error),
        }
    }
}
