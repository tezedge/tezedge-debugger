use core::{mem, ptr, convert::TryFrom};

pub struct DataDescriptor {
    pub id: EventId,
    pub tag: DataTag,
    pub size: i32,
}

#[derive(Debug, Clone)]
pub struct EventId {
    pub pid: u32,
    pub fd: u32,
}

impl TryFrom<&[u8]> for DataDescriptor {
    type Error = ();

    // TODO: rewrite safe
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        if v.len() >= mem::size_of::<Self>() {
            Ok(unsafe { ptr::read(v.as_ptr() as *const Self) })
        } else {
            Err(())
        }
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum DataTag {
    Write,
    SendTo,
    SendMsg,

    Read,
    RecvFrom,

    Connect,
    SocketName,
    Close,
}
