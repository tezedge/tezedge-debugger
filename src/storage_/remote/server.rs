use std::{
    future::Future,
    io,
    path::Path,
    fs,
    fmt,
    sync::Arc,
};
use tokio::{net::UnixListener, io::AsyncReadExt, task::JoinHandle};
use generic_array::{ArrayLength, GenericArray, typenum};
use rocksdb::WriteOptions;
use futures::{
    future::{self, FutureExt, Either},
    pin_mut,
};
use crate::storage_::local::LocalDb;
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
    inner: Arc<LocalDb>,
    cf_dictionary: Vec<&'static str>,
    listener: UnixListener,
}

impl DbServer {
    //const READER_CAPACITY: usize = 0x1000000;  // 16 MiB

    pub fn bind<P>(path: P, inner: &Arc<LocalDb>, cf_dictionary: Vec<&'static str>) -> io::Result<Self>
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

    pub fn spawn(self, terminate: impl Future<Output = ()> + Send + 'static) -> JoinHandle<()> {
        tokio::spawn(async move {
            let terminate = terminate.fuse();
            pin_mut!(terminate);
            loop {
                let accept_future = self.listener.accept();
                pin_mut!(accept_future);

                terminate = match future::select(accept_future, terminate).await {
                    Either::Left((Ok((stream, _)), terminate)) => {
                        match handle_connection(stream, self.inner.clone(), self.cf_dictionary.clone(), &Self::default_write_opts()).await {
                            Ok(()) => terminate,
                            Err(error) => {
                                tracing::error!(error = tracing::field::display(&error), "error handling connection");
                                terminate
                            },
                        }
                    },
                    Either::Left((Err(error), terminate)) => {
                        tracing::error!(error = tracing::field::display(&error), "error accepting connection");
                        terminate
                    },
                    Either::Right(((), _)) => break,
                }
            }
        })
    }
}

async fn handle_connection<S>(
    mut stream: S,
    inner: Arc<LocalDb>,
    cf_dictionary: Vec<&'static str>,
    write_opts: &WriteOptions,
) -> Result<(), DbServerError>
where
    S: AsyncReadExt + Unpin,
{
    loop {
        match handle_connection_inner(&mut stream, inner.clone(), &cf_dictionary, write_opts).await {
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

async fn handle_connection_inner<S>(
    stream: &mut S,
    inner: Arc<LocalDb>,
    cf_dictionary: &Vec<&'static str>,
    write_opts: &WriteOptions,
) -> Result<(), DbServerError>
where
    S: AsyncReadExt + Unpin,
{
    let index = read_u16(stream).await? as usize;
    let name = *cf_dictionary.get(index)
        .ok_or(DbServerError::ColumnIndex { allowed: cf_dictionary.len(), index })?;

    let op = read_u16(stream).await?;
    if DbRemoteOperation::Put as u16 == op {
        let key_size = read_u32(stream).await? as usize;
        if key_size > KEY_SIZE_LIMIT {
            Err(DbServerError::KeySize(key_size))?;
        }
        let value_size = read_u32(stream).await? as usize;
        if value_size > VALUE_SIZE_LIMIT {
            Err(DbServerError::ValueSize(value_size))?;
        }

        let mut big_buf = vec![0; key_size + value_size];
        stream.read_exact(&mut big_buf).await?;
        let key = &big_buf[0..key_size];
        let value = &big_buf[key_size..(key_size + value_size)];

        let cf = inner.as_ref().as_ref().cf_handle(name).ok_or(DbServerError::ColumnNotFound { name })?;
        inner.as_ref().as_ref().put_cf_opt(cf, key, value, &write_opts)?;

        // TODO: buf read
        /*let buf = future::poll_fn(|cx| buf_stream.poll_fill_buf(cx)).await?;
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
                    buf = future::poll_fn(|cx| buf_stream.poll_fill_buf(cx)).await?;
                    if buf.is_empty() {
                        Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""))?;
                    }
                }
            }
            let key = &big_buf[0..key_size];
            let value = &big_buf[key_size..(key_size + value_size)];
            inner.put_cf_opt(cf, key, value, &write_opts)?;
        }*/
    } else { // add operations here
        tracing::error!(op = op, "unsupported operation");
        Err(DbServerError::UnsupportedOperation(op))?;
    }

    Ok(())
}

async fn read_array<S, L>(s: &mut S) -> io::Result<GenericArray<u8, L>>
where
    S: AsyncReadExt + Unpin,
    L: ArrayLength<u8>,
{
    let mut a = GenericArray::default();
    let _ = s.read_exact(a.as_mut()).await?;
    Ok(a)
}

async fn read_u16<S>(s: &mut S) -> io::Result<u16>
where
    S: AsyncReadExt + Unpin,
{
    let a = read_array::<S, typenum::U2>(s).await?;
    Ok(u16::from_ne_bytes(a.into()))
}

async fn read_u32<S>(s: &mut S) -> io::Result<u32>
where
    S: AsyncReadExt + Unpin,
{
    let a = read_array::<S, typenum::U4>(s).await?;
    Ok(u32::from_ne_bytes(a.into()))
}
