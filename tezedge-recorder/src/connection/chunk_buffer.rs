// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub struct Buffer {
    counter: u64,
    ptr: *mut libc::c_void,
    len: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            counter: 0,
            ptr: std::ptr::null_mut(),
            len: 0,
        }
    }
}

impl Buffer {
    pub fn handle_data(&mut self, payload: &[u8]) {
        unsafe {
            if self.ptr.is_null() {
                assert_eq!(self.len, 0);
                self.ptr = libc::malloc(payload.len());
            } else {
                self.ptr = libc::realloc(self.ptr, self.len + payload.len());
            }
            libc::memcpy(
                self.ptr.offset(self.len as isize),
                payload.as_ptr() as *mut _,
                payload.len(),
            );
            self.len += payload.len()
        }
    }

    fn len(&self) -> Option<usize> {
        if self.len < 2 {
            return None;
        }
        assert!(!self.ptr.is_null());
        unsafe {
            let b0 = *(self.ptr as *mut u8);
            let b1 = *((self.ptr as *mut u8).offset(1));
            Some((b0 as usize) * 256 + (b1 as usize))
        }
    }

    pub fn have_chunk(&self) -> bool {
        self.len >= 2 + self.len().unwrap_or(0)
    }
}

impl Iterator for Buffer {
    type Item = (u64, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.len()? + 2;
        if self.len < len {
            None
        } else {
            let counter = self.counter;
            self.counter += 1;

            let mut new = vec![0; len];
            unsafe {
                libc::memcpy(
                    new.as_mut_ptr() as *mut _,
                    self.ptr as *const _,
                    len,
                );

                self.len -= len;
                if self.len > len {
                    let remaining = libc::malloc(self.len);
                    libc::memcpy(
                        remaining,
                        self.ptr.offset(len as isize) as *const _,
                        self.len,
                    );
                    libc::free(self.ptr);
                    self.ptr = remaining;
                } else {
                    libc::free(self.ptr);
                    self.ptr = std::ptr::null_mut();
                }
            }

            Some((counter, new))
        }
    }
}
