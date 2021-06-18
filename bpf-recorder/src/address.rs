// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ebpf_kern::helpers;

pub struct Address {
    sa_family: u16,
    port: u16,
}

impl Address {
    const AF_INET: u16 = 2;
    const AF_INET6: u16 = 10;

    #[inline(always)]
    pub fn read(addr_ptr: u64, addr_len: u64) -> Result<Self, i32> {
        if addr_len < 4 {
            return Err(-1);
        }

        let mut address = Address {
            sa_family: 0,
            port: 0,
        };
        let c = unsafe {
            helpers::probe_read_user(&mut address as *mut Address as _, 4, addr_ptr as *const _)
        };
        if c < 0 {
            return Err(c as _);
        }
        if address.sa_family != Self::AF_INET && address.sa_family != Self::AF_INET6 {
            return Err(-1);
        }

        Ok(address)
    }

    #[inline(always)]
    pub fn port(&self) -> u16 {
        self.port
    }
}
