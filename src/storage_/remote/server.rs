use std::{
    os::unix::net::UnixListener,
    io::{self, BufRead, BufReader},
    path::Path,
    fs,
    fmt,
    sync::Arc,
    thread::{self, JoinHandle},
};
use generic_array::{ArrayLength, GenericArray, typenum};
use rocksdb::{DB, WriteOptions};
use super::common::{DbRemoteOperation, KEY_SIZE_LIMIT, VALUE_SIZE_LIMIT};

pub enum DbServerError {
    Io(io::Error),
    ColumnIndex {
        allowed: usize,
        index: usize,
    },
    ColumnNotFound {
        name: &'static str,
    },
    RocksDb(rocksdb::Error),
    UnsupportedOperation(u16),
    KeySize(usize),
    ValueSize(usize),
}

impl From<io::Error> for DbServerError {
    fn from(v: io::Error) -> Self {
        DbServerError::Io(v)
    }
}

impl From<rocksdb::Error> for DbServerError {
    fn from(v: rocksdb::Error) -> Self {
        DbServerError::RocksDb(v)
    }
}

impl fmt::Display for DbServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &DbServerError::Io(ref error) => write!(f, "io error: {}", error),
            &DbServerError::ColumnIndex { allowed, index } => {
                write!(f, "column index {} out of range 0..{} ", index, allowed)
            },
            &DbServerError::ColumnNotFound { name } => write!(f, "column not found {}", name),
            &DbServerError::RocksDb(ref error) => write!(f, "rocksdb error: {}", error),
            &DbServerError::UnsupportedOperation(code) => {
                write!(f, "unsupported operation code {}", code)
            },
            &DbServerError::KeySize(size) => {
                write!(f, "key is too big {}, limit {}", size, KEY_SIZE_LIMIT)
            },
            &DbServerError::ValueSize(size) => {
                write!(f, "value is too big {}, limit {}", size, VALUE_SIZE_LIMIT)
            },
        }
    }
}

impl DbServerError {
    fn eof(&self) -> bool {
        match self {
            &DbServerError::Io(ref error) => {
                error.kind() == io::ErrorKind::UnexpectedEof
            },
            _ => false,
        }
    }
}

pub struct DbServer {
    inner: Arc<DB>,
    cf_dictionary: Vec<&'static str>,
    listener: UnixListener,
}

impl DbServer {
    const READER_CAPACITY: usize = 0x1000000;  // 16 MiB

    pub fn bind<P>(path: P, inner: &Arc<DB>, cf_dictionary: Vec<&'static str>) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;

        Ok(DbServer {
            inner: inner.clone(),
            cf_dictionary,
            listener,
        })
    }

    fn default_write_opts() -> WriteOptions {
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(false);
        write_opts
    }

    pub fn spawn(self) -> JoinHandle<Result<(), DbServerError>> {
        thread::spawn(move || {
            // let's serve single client
            let (stream, _) = self.listener.accept()?;
            let mut buf_stream = BufReader::with_capacity(Self::READER_CAPACITY, stream);
            handle_connection(&mut buf_stream, self.inner.clone(), self.cf_dictionary, &Self::default_write_opts())
        })    
    }
}

fn handle_connection<S>(
    buf_stream: &mut S,
    inner: Arc<DB>,
    cf_dictionary: Vec<&'static str>,
    write_opts: &WriteOptions,
) -> Result<(), DbServerError>
where
    S: BufRead,
{
    loop {
        match handle_connection_inner(buf_stream, inner.clone(), &cf_dictionary, write_opts) {
            Ok(()) => (),
            Err(e) => {
                if e.eof() {
                    break Ok(());
                } else {
                    break Err(e);
                }
            }
        }
    }
}

fn handle_connection_inner<S>(
    buf_stream: &mut S,
    inner: Arc<DB>,
    cf_dictionary: &Vec<&'static str>,
    write_opts: &WriteOptions,
) -> Result<(), DbServerError>
where
    S: BufRead,
{
    let index = buf_stream.read_u16()? as usize;
    let name = *cf_dictionary.get(index)
        .ok_or(DbServerError::ColumnIndex { allowed: cf_dictionary.len(), index })?;
    let cf = inner.cf_handle(name).ok_or(DbServerError::ColumnNotFound { name })?;

    let op = buf_stream.read_u16()?;
    if DbRemoteOperation::Put as u16 == op {
        let key_size = buf_stream.read_u32()? as usize;
        if key_size > KEY_SIZE_LIMIT {
            Err(DbServerError::KeySize(key_size))?;
        }
        let value_size = buf_stream.read_u32()? as usize;
        if value_size > KEY_SIZE_LIMIT {
            Err(DbServerError::ValueSize(value_size))?;
        }

        let buf = buf_stream.fill_buf()?;
        if buf.is_empty() {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""))?;
        }
        let target = key_size + value_size;
        if buf.len() >= target {
            let key = &buf[0..key_size];
            let value = &buf[key_size..(key_size + value_size)];
            inner.put_cf_opt(cf, key, value, &write_opts)?;
            buf_stream.consume(target);
        } else {
            let mut big_buf = Vec::with_capacity(target);
            let mut buf = buf;
            loop {
                if big_buf.len() + buf.len() >= target {
                    let local_target = target - big_buf.len();
                    big_buf.extend_from_slice(&buf[0..local_target]);
                    buf_stream.consume(local_target);
                    break;
                } else {
                    let local_target = buf.len();
                    big_buf.extend_from_slice(buf);
                    buf_stream.consume(local_target);
                    buf = buf_stream.fill_buf()?;
                    if buf.is_empty() {
                        Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""))?;
                    }
                }
            }
            let key = &big_buf[0..key_size];
            let value = &big_buf[key_size..(key_size + value_size)];
            inner.put_cf_opt(cf, key, value, &write_opts)?;
        }
    } else { // add operations here
        tracing::error!(op = op, "unsupported operation");
        Err(DbServerError::UnsupportedOperation(op))?;
    }

    Ok(())
}

trait BufReadArray {
    fn read_array<L>(&mut self) -> io::Result<GenericArray<u8, L>>
    where
        L: ArrayLength<u8>;
}

impl<T> BufReadArray for T
where
    T: BufRead,
{
    fn read_array<L>(&mut self) -> io::Result<GenericArray<u8, L>>
    where
        L: ArrayLength<u8>,
    {
        let buf = self.fill_buf()?;
        if buf.len() < L::USIZE {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""))
        } else {
            let mut a = GenericArray::default();
            a.clone_from_slice(&buf[0..L::USIZE]);
            self.consume(L::USIZE);
            Ok(a)
        }
    }
}

trait BufReadArrayExt {
    fn read_u16(&mut self) -> io::Result<u16>;
    fn read_u32(&mut self) -> io::Result<u32>;
}

impl<T> BufReadArrayExt for T
where
    T: BufReadArray,
{
    fn read_u16(&mut self) -> io::Result<u16> {
        let a = self.read_array::<typenum::U2>()?;
        Ok(u16::from_ne_bytes(a.into()))
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        let a = self.read_array::<typenum::U4>()?;
        Ok(u32::from_ne_bytes(a.into()))
    }
}
