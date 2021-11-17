use std::{
    fs,
    ops::DerefMut,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard, TryLockError,
        atomic::{Ordering, AtomicBool},
    },
    thread,
};
use tantivy::{
    directory::MmapDirectory, schema, Index, IndexWriter, Document, ReloadPolicy,
    query::QueryParser, collector::TopDocs, TantivyError,
};

pub struct LogIndexer {
    index: Index,
    writer_thread: Option<thread::JoinHandle<()>>,
    commit_state: Arc<CommitState>,
}

#[derive(Default)]
struct DocumentQueue {
    start_id: u64,
    messages: Vec<String>,
}

impl DocumentQueue {
    fn enqueue(&mut self, id: u64, msg: &str) {
        if self.messages.is_empty() {
            self.start_id = id;
        }
        self.messages.push(msg.to_string());
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn drain(self) -> impl Iterator<Item = (u64, String)> {
        let id = self.start_id;
        self.messages
            .into_iter()
            .enumerate()
            .map(move |(i, msg)| (id + (i as u64), msg))
    }
}

struct CommitState {
    id_field: schema::Field,
    message_field: schema::Field,
    queue: Arc<Mutex<DocumentQueue>>,
    dirty: AtomicBool,
    running: AtomicBool,
    writer: Mutex<IndexWriter>,
}

impl CommitState {
    fn run(&self) {
        use std::time::{Instant, Duration};

        const TICK: Duration = Duration::from_millis(1000);

        fn align(duration: Duration, tick: Duration) -> Duration {
            let duration = duration.as_micros() as u64;
            let tick = tick.as_micros() as u64;
            Duration::from_micros(tick - (duration % tick))
        }

        loop {
            let commit_begin = Instant::now();
            if self.dirty.fetch_and(false, Ordering::SeqCst) {
                let mut writer = self.finalize();
                match writer.commit() {
                    Ok(_) => (),
                    Err(error) => log::error!("cannot commit log index: {:?}", error),
                }
            }
            if !self.running.load(Ordering::Relaxed) {
                break;
            }
            let commit_end = Instant::now();
            let elapsed = commit_end.duration_since(commit_begin);
            thread::sleep(align(elapsed, TICK));
        }
    }

    fn finalize(&self) -> MutexGuard<IndexWriter> {
        use std::mem;

        let writer = self.writer.lock().unwrap();
        let mut queue_lock = self.queue.lock().unwrap();
        if !queue_lock.is_empty() {
            let queue = mem::replace(queue_lock.deref_mut(), DocumentQueue::default());
            drop(queue_lock);
            for (id, message) in queue.drain() {
                writer.add_document(self.prepare_doc(&message, id));
            }
            self.dirty.fetch_or(true, Ordering::SeqCst);
        }
        writer
    }

    fn prepare_doc(&self, message: &str, id: u64) -> Document {
        let mut doc = Document::default();
        doc.add_text(self.message_field, message);
        doc.add_u64(self.id_field, id);
        doc
    }
}

impl Drop for LogIndexer {
    fn drop(&mut self) {
        if let Some(log_writer) = self.writer_thread.take() {
            self.commit_state.running.store(false, Ordering::Relaxed);
            match log_writer.join() {
                Ok(()) => (),
                Err(error) => log::error!("error joining log indexer thread: {:?}", error),
            }
        }
    }
}

impl LogIndexer {
    const HEAP_BYTES: usize = 32 * 1024 * 1024; // 32Mb

    pub fn try_new<P>(path: P) -> Result<Self, TantivyError>
    where
        P: AsRef<Path>,
    {
        let mut schema_builder = schema::Schema::builder();
        schema_builder.add_text_field("message", schema::TEXT);
        schema_builder.add_text_field("id", schema::STORED);
        let schema = schema_builder.build();

        let _ = fs::create_dir_all(&path);
        let index = Index::open_or_create(MmapDirectory::open(path)?, schema.clone())?;
        let message_field = schema.get_field("message").unwrap();
        let id_field = schema.get_field("id").unwrap();
        let queue = Default::default();
        let commit_state = Arc::new(CommitState {
            dirty: AtomicBool::new(false),
            running: AtomicBool::new(true),
            writer: Mutex::new(index.writer(Self::HEAP_BYTES)?),
            id_field,
            message_field,
            queue,
        });

        let writer_thread = {
            let commit_state = commit_state.clone();
            Some(thread::spawn(move || commit_state.run()))
        };

        Ok(LogIndexer {
            index,
            writer_thread,
            commit_state,
        })
    }

    pub fn write(&self, message: &str, id: u64) {
        use std::mem;

        match self.commit_state.writer.try_lock() {
            Ok(writer) => {
                let mut queue_lock = self.commit_state.queue.lock().unwrap();
                let queue = mem::replace(queue_lock.deref_mut(), DocumentQueue::default());
                drop(queue_lock);
                for (id, message) in queue.drain() {
                    writer.add_document(self.commit_state.prepare_doc(&message, id));
                }
                writer.add_document(self.commit_state.prepare_doc(message, id));
                self.commit_state.dirty.fetch_or(true, Ordering::SeqCst);
            },
            Err(TryLockError::Poisoned(e)) => Err::<(), _>(e).unwrap(),
            Err(TryLockError::WouldBlock) => {
                self.commit_state.queue.lock().unwrap().enqueue(id, message)
            },
        }
    }

    pub fn read(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<impl Iterator<Item = (f32, u64)>, TantivyError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;
        let searcher = reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.commit_state.message_field]);
        let query = query_parser.parse_query(query)?;
        let id_field = self.commit_state.id_field;
        let it = searcher
            .search(&query, &TopDocs::with_limit(limit))?
            .into_iter()
            .filter_map(move |(score, doc_address)| {
                let retrieved_doc = searcher.doc(doc_address).ok()?;
                let f = retrieved_doc
                    .field_values()
                    .iter()
                    .find(|x| x.field() == id_field)?;
                match f.value() {
                    &schema::Value::U64(ref id) if score > 0.0 => Some((score, *id)),
                    _ => None,
                }
            });
        Ok(it)
    }
}
