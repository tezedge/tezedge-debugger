use redbpf_probes::{maps::HashMap, helpers};
use super::data_descriptor::{SocketId, EventId};

#[derive(Clone)]
pub enum SyscallContext {
    Empty {
        fake_fd: u32,
        fake_data: &'static [u8],
    },

    Write {
        fd: u32,
        data_ptr: usize,
    },
    SendTo {
        fd: u32,
        data_ptr: usize,
    },
    SendMsg {
        fd: u32,
        message: &'static [u8],
    },

    Read {
        fd: u32,
        data_ptr: usize,
    },
    RecvFrom {
        fd: u32,
        data_ptr: usize,
    },

    Connect {
        fd: u32,
        address: &'static [u8],
    },
    SocketName {
        fd: u32,
        address: &'static [u8],
    },
}

impl SyscallContext {
    /// bpf validator forbids reading from stack uninitialized data
    /// different variants of this enum has different length,
    /// `Empty` variant should be biggest
    #[inline(always)]
    pub fn empty() -> Self {
        SyscallContext::Empty {
            fake_fd: 0,
            fake_data: b"",
        }
    }

    #[inline(always)]
    pub fn push(self, map: &mut HashMap<u64, SyscallContext>) {
        let id = helpers::bpf_get_current_pid_tgid();
        map.set(&id, &self)
    }

    #[inline(always)]
    pub fn pop_with<F>(map: &mut HashMap<u64, SyscallContext>, f: F)
    where
        F: FnOnce(Self),
    {
        let id = helpers::bpf_get_current_pid_tgid();
        match map.get(&id) {
            Some(context) => {
                f(context.clone());
                map.delete(&id);
            },
            None => (),
        }
    }
}
