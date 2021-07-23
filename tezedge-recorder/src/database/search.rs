use std::{
    fs,
    ops::DerefMut,
    path::Path,
    sync::{Arc, Mutex, TryLockError, atomic::{Ordering, AtomicBool}},
    thread,
};
use tantivy::{
    directory::MmapDirectory,
    schema,
    Index,
    IndexWriter,
    Document, 
    ReloadPolicy,
    query::QueryParser,
    collector::TopDocs,
    TantivyError,
};

pub struct LogIndexer {
    index: Index,
    id_field: schema::Field,
    message_field: schema::Field,
    writer_thread: Option<thread::JoinHandle<()>>,
    queue: Arc<Mutex<(u64, Vec<String>)>>,
    commit_state: Arc<State>,
}

struct State {
    dirty: AtomicBool,
    running: AtomicBool,
    writer: Mutex<IndexWriter>,
}

impl State {
    fn run(&self) {
        use std::time::{Instant, Duration};

        const TICK: Duration = Duration::from_millis(1000);

        fn align(duration: Duration, tick: Duration) -> Duration {
            let duration = duration.as_micros() as u64;
            let tick = tick.as_micros() as u64;
            Duration::from_micros(tick - (duration % tick))
        }

        while self.running.load(Ordering::Relaxed) {
            let commit_begin = Instant::now();
            if self.dirty.fetch_and(false, Ordering::SeqCst) {
                match self.writer.lock().unwrap().commit() {
                    Ok(_) => (),
                    Err(error) => log::error!("cannot commit log index: {:?}", error),
                }
            }
            let commit_end = Instant::now();
            let elapsed = commit_end.duration_since(commit_begin);
            thread::sleep(align(elapsed, TICK));
        }
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
        let commit_state = Arc::new(State {
            dirty: AtomicBool::new(false),
            running: AtomicBool::new(true),
            writer: Mutex::new(index.writer(Self::HEAP_BYTES)?),
        });

        let writer_thread = {
            let commit_state = commit_state.clone();
            Some(thread::spawn(move || commit_state.run()))
        };

        Ok(LogIndexer { index, message_field, id_field, queue, writer_thread, commit_state })
    }

    pub fn write(&self, message: &str, id: u64) {
        use std::mem;

        match self.commit_state.writer.try_lock() {
            Ok(writer) => {
                let (base, mut queue) = mem::replace(self.queue.lock().unwrap().deref_mut(), (0, Vec::new()));
                queue.push(message.to_string());
                for (i, message) in queue.into_iter().enumerate() {
                    let mut doc = Document::default();
                    doc.add_text(self.message_field, message);
                    doc.add_u64(self.id_field, base + (i as u64));
                    writer.add_document(doc);
                }
                self.commit_state.dirty.fetch_or(true, Ordering::SeqCst);
            },
            Err(TryLockError::Poisoned(e)) => Err::<(), _>(e).unwrap(),
            Err(TryLockError::WouldBlock) => {
                let mut queue = self.queue.lock().unwrap();
                if queue.1.is_empty() {
                    queue.0 = id;
                }
                queue.1.push(message.to_string());
            }
        }
    }

    pub fn read(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<impl Iterator<Item = (f32, u64)>, TantivyError> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.message_field]);
        let query = query_parser.parse_query(query)?;
        let id_field = self.id_field;
        let it = searcher
            .search(&query, &TopDocs::with_limit(limit))?
            .into_iter()
            .filter_map(move |(score, doc_address)| {
                let retrieved_doc = searcher.doc(doc_address).ok()?;
                let f = retrieved_doc.field_values().iter()
                    .find(|x| x.field() == id_field)?;
                match f.value() {
                    &schema::Value::U64(ref id) => Some((score, *id)),
                    _ => None,
                }
            });
        Ok(it)
    }
}
