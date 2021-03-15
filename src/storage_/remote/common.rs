
#[repr(u16)]
pub enum DbRemoteOperation {
    Put,
}

pub const KEY_SIZE_LIMIT: usize = 0x1000;  // 4 KiB
pub const VALUE_SIZE_LIMIT: usize = 0x8000000;  // 128 MiB
