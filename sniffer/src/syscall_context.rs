use redbpf_probes::{maps::HashMap, helpers};
use super::data_descriptor::EventId;

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
    pub fn empty() -> Self {
        SyscallContext::Empty {
            fake_fd: 0,
            fake_data: b"",
        }
    }

    fn fd(&self) -> u32 {
        match self {
            &SyscallContext::Empty { ref fake_fd, .. } => *fake_fd,
            &SyscallContext::Write { ref fd, .. } => *fd,
            &SyscallContext::SendTo { ref fd, .. } => *fd,
            &SyscallContext::SendMsg { ref fd, .. } => *fd,
            &SyscallContext::Read { ref fd, .. } => *fd,
            &SyscallContext::RecvFrom { ref fd, .. } => *fd,
            &SyscallContext::Connect { ref fd, .. } => *fd,
            &SyscallContext::SocketName { ref fd, .. } => *fd,
        }
    }

    pub fn push(self, map: &mut HashMap<u64, SyscallContext>) {
        let id = helpers::bpf_get_current_pid_tgid();
        map.set(&id, &self)
    }

    pub fn pop_with<F>(map: &mut HashMap<u64, SyscallContext>, f: F)
    where
        F: FnOnce(Self, EventId),
    {
        let id = helpers::bpf_get_current_pid_tgid();
        match map.get(&id) {
            Some(context) => {
                let eid = EventId {
                    pid: (id & 0xffffffff) as u32,
                    fd: context.fd(),
                };
                f(context.clone(), eid);
                map.delete(&id);
            },
            None => (),
        }
    }
}
