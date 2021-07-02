// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg_attr(feature = "kern", no_std, no_main, feature(lang_items))]

#[cfg(feature = "kern")]
use ebpf_kern as ebpf;
#[cfg(feature = "user")]
use ebpf_user as ebpf;

#[cfg(feature = "kern")]
ebpf::license!("Dual MIT/GPL");

#[cfg(any(feature = "kern", feature = "user"))]
#[derive(ebpf::BpfApp)]
pub struct App {
    #[hashmap(size = 1)]
    pub pid: ebpf::HashMapRef<4, 4>,
    #[hashmap(size = 1)]
    pub lost_events: ebpf::HashMapRef<4, 4>,
    #[array_percpu(size = 1)]
    pub stack: ebpf::ArrayPerCpuRef<0x400>,
    #[ringbuf(size = 0x8000000)]
    pub event_queue: ebpf::RingBufferRef,
    #[prog("tracepoint/syscalls/sys_enter_execve")]
    pub execve: ebpf::ProgRef,
    #[prog("tracepoint/syscalls/sys_enter_execveat")]
    pub execveat: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_alloc")]
    pub page_alloc: ebpf::ProgRef,
    #[prog("tracepoint/kmem/mm_page_free")]
    pub page_free: ebpf::ProgRef,
    #[prog("tracepoint/kmem/rss_stat")]
    pub rss_stat: ebpf::ProgRef,
    #[prog("tracepoint/filemap/mm_filemap_add_to_page_cache")]
    pub add_to_page_cache: ebpf::ProgRef,
    #[prog("tracepoint/filemap/mm_filemap_delete_from_page_cache")]
    pub remove_from_page_cache: ebpf::ProgRef,
    #[prog("tracepoint/migrate/mm_migrate_pages")]
    pub migrate_pages: ebpf::ProgRef,
}

#[cfg(feature = "kern")]
use {
    bpf_memprof_common::{Pod, STACK_MAX_DEPTH},
    bpf_memprof_common::{
        KFree, KMAlloc, KMAllocNode, CacheAlloc, CacheAllocNode, CacheFree, PageAlloc, PageFree,
        PageFreeBatched, RssStat, PercpuAlloc, PercpuFree, AddToPageCache, RemoveFromPageCache,
        MigratePages,
    },
    ebpf::helpers,
};

#[cfg(feature = "kern")]
impl App {
    #[inline(always)]
    fn check_no_pid(&self) -> Result<(), i32> {
        if let Some(&pid_bytes) = self.pid.get(&0u32.to_ne_bytes()) {
            let target_pid = u32::from_ne_bytes(pid_bytes);

            if target_pid != 0 {
                return Err(0);
            }
        }

        Ok(())
    }

    #[inline(always)]
    fn check_pid(&self) -> Result<u32, i32> {
        if let Some(&pid_bytes) = self.pid.get(&0u32.to_ne_bytes()) {
            let target_pid = u32::from_ne_bytes(pid_bytes);

            let x = unsafe { helpers::get_current_pid_tgid() };
            let pid = (x >> 32) as u32;
            if pid != target_pid {
                Err(0)
            } else {
                Ok(pid)
            }
        } else {
            Err(0)
        }
    }

    #[inline(always)]
    fn check_filename(&mut self, filename_ptr: *const u8) -> Result<(), i32> {
        if filename_ptr.is_null() {
            return Err(0);
        }

        let mut buffer = self.event_queue.reserve(0x200)?;
        let c = unsafe {
            helpers::probe_read_user_str(
                buffer.as_mut().as_mut_ptr() as _,
                0x200,
                filename_ptr as _,
            )
        };

        let pos = if c < 11 || c > 0x200 {
            buffer.discard();
            return Err(c as _);
        } else {
            c as usize - 11
        };

        let buffer_ref = &buffer.as_ref()[pos..];
        let pass = true
            && buffer_ref[0] == 'l' as u8
            && buffer_ref[1] == 'i' as u8
            && buffer_ref[2] == 'g' as u8
            && buffer_ref[3] == 'h' as u8
            && buffer_ref[4] == 't' as u8
            && buffer_ref[5] == '-' as u8
            && buffer_ref[6] == 'n' as u8
            && buffer_ref[7] == 'o' as u8
            && buffer_ref[8] == 'd' as u8
            && buffer_ref[9] == 'e' as u8;
        buffer.discard();

        if pass {
            let x = unsafe { helpers::get_current_pid_tgid() };
            let pid = (x >> 32) as u32;
            self.pid.insert(0u32.to_ne_bytes(), pid.to_ne_bytes())?;
        }
        Ok(())
    }

    #[inline(always)]
    pub fn execve(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.check_no_pid()?;

        self.check_filename(ctx.read_here::<*const u8>(0x10))
    }

    #[inline(always)]
    pub fn execveat(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.check_no_pid()?;

        self.check_filename(ctx.read_here::<*const u8>(0x18))
    }

    #[inline(always)]
    fn inc_lost(&mut self) -> Result<(), i32> {
        if let Some(cnt_bytes) = self.lost_events.get_mut(&0u32.to_ne_bytes()) {
            if cnt_bytes[0] < u8::MAX {
                cnt_bytes[0] += 1;
            } else if cnt_bytes[1] < u8::MAX {
                cnt_bytes[0] = 0;
                cnt_bytes[1] += 1;
            } else if cnt_bytes[2] < u8::MAX {
                cnt_bytes[0] = 0;
                cnt_bytes[1] = 0;
                cnt_bytes[2] += 1;
            } else if cnt_bytes[3] < u8::MAX {
                cnt_bytes[0] = 0;
                cnt_bytes[1] = 0;
                cnt_bytes[2] = 0;
                cnt_bytes[3] += 1;
            }
            Ok(())
        } else {
            self.lost_events.insert(0u32.to_ne_bytes(), 1u32.to_le_bytes())
        }
    }

    #[inline(always)]
    fn output_unconditional<T>(&mut self, ctx: ebpf::Context) -> Result<(), i32>
    where
        T: Pod,
    {
        let x = unsafe { helpers::get_current_pid_tgid() };
        let pid = (x >> 32) as u32;
        self.output_generic::<T>(ctx, pid, false)
    }

    #[inline(always)]
    fn output<T>(&mut self, ctx: ebpf::Context, need_stack: bool) -> Result<(), i32>
    where
        T: Pod,
    {
        let pid = self.check_pid()?;
        self.output_generic::<T>(ctx, pid, need_stack)
    }

    #[inline(always)]
    fn output_generic<T>(
        &mut self,
        ctx: ebpf::Context,
        pid: u32,
        need_stack: bool,
    ) -> Result<(), i32>
    where
        T: Pod,
    {
        let stack_len = if need_stack {
            self.stack.get_mut(0)
                .map(|s| {
                    let size = ctx.get_user_stack(s).unwrap_or(0);
                    (size / 8) as usize
                })
                .unwrap_or(0)
        } else {
            0
        };
        

        let stack_len = if stack_len > 64 {
            STACK_MAX_DEPTH
        } else if stack_len > 32 {
            64
        } else if stack_len > 16 {
            32
        } else if stack_len > 8 {
            16
        } else if stack_len > 4 {
            8
        } else if stack_len > 2 {
            4
        } else if stack_len > 1 {
            2
        } else if stack_len > 0 {
            1
        } else {
            0
        };

        let size = 0x10 + T::SIZE + 0x08 + stack_len * 8;
        let mut data = self.event_queue.reserve(size)
            .map_err(|e| {
                let _ = self.inc_lost();
                e
            })?;
        let data_mut = data.as_mut();
        ctx.read_into(0x00, &mut data_mut[..0x08]);
        data_mut[0x08..0x0c].clone_from_slice(&pid.to_ne_bytes());
        data_mut[0x0c..0x10].clone_from_slice(&T::DISCRIMINANT.unwrap_or(0).to_ne_bytes());
        let data_mut = &mut data_mut[0x10..];
        ctx.read_into(0x08, &mut data_mut[..T::SIZE]);
        let data_mut = &mut data_mut[T::SIZE..];
        data_mut[..0x08].clone_from_slice(&(stack_len as u64).to_ne_bytes());
        if !need_stack {
            data.submit();
            return Ok(());
        }
        match ctx.get_user_stack(&mut data_mut[0x08..]) {
            Ok(size) => {
                data.submit();
                Ok(())
            },
            Err(e) => {
                data.submit();
                Err(e)
            },
        }
    }

    // /sys/kernel/debug/tracing/events/kmem/mm_page_alloc/format

    #[allow(dead_code)]
    #[inline(always)]
    pub fn kfree(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<KFree>(ctx)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn kmalloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<KMAlloc>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn kmalloc_node(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<KMAllocNode>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn cache_alloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<CacheAlloc>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn cache_alloc_node(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<CacheAllocNode>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn cache_free(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<CacheFree>(ctx)
    }

    #[inline(always)]
    pub fn page_alloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<PageAlloc>(ctx, true)
    }

    #[inline(always)]
    pub fn page_free(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<PageFree>(ctx)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn page_free_batched(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<PageFreeBatched>(ctx)
    }

    #[inline(always)]
    pub fn rss_stat(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<RssStat>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn percpu_alloc(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output::<PercpuAlloc>(ctx, false)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn percpu_free(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<PercpuFree>(ctx)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn add_to_page_cache(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<AddToPageCache>(ctx)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn remove_from_page_cache(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<RemoveFromPageCache>(ctx)
    }

    #[inline(always)]
    pub fn migrate_pages(&mut self, ctx: ebpf::Context) -> Result<(), i32> {
        self.output_unconditional::<MigratePages>(ctx)
    }
}

#[cfg(feature = "user")]
#[allow(dead_code)]
fn accept_client() -> std::os::unix::net::UnixStream {
    use std::{fs, os::unix::{fs::PermissionsExt, net::UnixListener}};

    let socket = "/tmp/bpf-memprof.sock";
    let _ = fs::remove_file(socket);
    let _ = fs::create_dir("/tmp");
    let listener = UnixListener::bind(socket).expect("failed to bind socket");

    let mut perms = fs::metadata(socket)
        .expect("failed to get permission of socket")
        .permissions();
    perms.set_mode(0o666);
    fs::set_permissions(socket, perms).expect("failed to set permission for socket");

    let (stream, address) = listener.accept().expect("failed to accept connection");
    log::info!("accept client: {:?}", address);
    stream
}

#[cfg(feature = "user")]
fn run_bpf() -> (ebpf::Skeleton<App>, i32) {
    use std::io::Error;
    use ebpf::{Skeleton, kind::{AppItemKindMut, AppItem}};

    static CODE: &[u8] = include_bytes!(concat!("../", env!("BPF_CODE")));

    let mut skeleton = Skeleton::<App>::open("bpf-memprof\0", CODE)
        .unwrap_or_else(|code| panic!("failed to open bpf: {}", code));
    skeleton.load()
        .unwrap_or_else(|code| panic!("failed to load bpf: {}", code));
    if let Some(old_pid) = skeleton.app.pid.get(&0u32.to_ne_bytes()) {
        let old_pid = u32::from_ne_bytes(old_pid);
        if old_pid != 0 {
            log::warn!("detected old pid: {}", old_pid);
            match skeleton.app.pid.remove(&0u32.to_ne_bytes()) {
                Ok(()) => (),
                Err(code) => {
                    log::error!(
                        "failed to remove old pid, code {}, error {}",
                        code,
                        Error::last_os_error(),
                    );
                }
            }
        }
    }
    skeleton.attach()
        .unwrap_or_else(|code| panic!("failed to attach bpf: {}", code));
    log::info!("attached bpf module");

    let fd = match skeleton.app.event_queue.kind_mut() {
        AppItemKindMut::Map(map) => map.fd(),
        _ => unreachable!(),
    };

    (skeleton, fd)
}

#[cfg(feature = "user")]
fn main() {
    use std::{time::Duration, io, sync::{Arc, atomic::{Ordering, AtomicBool}}};
    use tracing::Level;
    use ebpf::RingBufferRegistry;
    use tezedge_memprof::{Consumer, StackResolver, server};
    //use passfd::FdPassingExt;

    sudo::escalate_if_needed().expect("failed to obtain superuser permission");
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // spawn a thread listening ctrl+c
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || running.store(false, Ordering::Relaxed))
            .expect("failed to setup ctrl+c handler");
    }

    // attack bpf module and acquire fd of event stream
    let (skeleton, fd) = run_bpf();

    /*let stream = accept_client();
    stream
        .send_fd(fd)
        .expect("failed to send ring buffer access");*/

    let cli = Consumer::default();
    let aggregator = cli.reporter();

    if std::env::args().find(|s| s == "--dump").is_some() {
        aggregator.lock().unwrap().turn_on_dump();
    }

    // spawn a thread monitoring process map from `/proc/<pid>/maps` and loading symbol tables
    let resolver = StackResolver::spawn(cli.pid());

    // spawn a thread-pool serving http requests, using tokio
    let server = server::run(cli.reporter(), resolver, cli.pid());

    let mut rb = RingBufferRegistry::default();
    let mut cli = cli;
    rb.add_fd(fd, move |data| cli.arrive(data))
        .map_err(|_| io::Error::last_os_error())
        .expect("failed to setup ring buffer");

    let mut overall_cnt = 0;
    let mut old_cnt = 0;
    while running.load(Ordering::Relaxed) {
        match rb.poll(Duration::from_secs(1)) {
            Ok(_) => {
                overall_cnt += 1;
                if overall_cnt & 0xffff == 0 {
                    let cnt = skeleton.app.lost_events.get(&0u32.to_ne_bytes())
                        .map(u32::from_le_bytes)
                        .unwrap_or(0);
                    if cnt - old_cnt != 0 {
                        log::warn!("lost events: {}", cnt - old_cnt);
                        old_cnt = cnt;
                    } else {
                        log::debug!("check: ok");
                    }
                }
            },
            Err(c) => {
                if c != -4 {
                    log::error!("code: {}, error: {}", c, io::Error::last_os_error());
                    break;
                }
            },
        }
    }

    aggregator.lock().unwrap().store_dump();
    log::info!("stop server");
    let _ = server;
}
