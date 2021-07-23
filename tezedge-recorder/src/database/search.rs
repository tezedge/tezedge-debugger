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
};

pub struct LogIndexer {
    index: Index,
    id_field: schema::Field,
    message_field: schema::Field,
    writer: Arc<Mutex<IndexWriter>>,
    queue: Arc<Mutex<(u64, Vec<String>)>>,
    writer_thread: Option<thread::JoinHandle<()>>,
    dirty: Arc<AtomicBool>,
}

impl Drop for LogIndexer {
    fn drop(&mut self) {
        if let Some(log_writer) = self.writer_thread.take() {
            log_writer.join().unwrap();
        }
    }
}

impl LogIndexer {
    const HEAP_BYTES: usize = 32 * 1024 * 1024; // 32Mb

    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut schema_builder = schema::Schema::builder();
        schema_builder.add_text_field("message", schema::TEXT);
        schema_builder.add_text_field("id", schema::STORED);
        let schema = schema_builder.build();

        let _ = fs::create_dir_all(&path);
        let index = Index::open_or_create(MmapDirectory::open(path).unwrap(), schema.clone()).unwrap();
        let message_field = schema.get_field("message").unwrap();
        let id_field = schema.get_field("id").unwrap();
        let writer = Arc::new(Mutex::new(index.writer(Self::HEAP_BYTES).unwrap()));
        let queue = Default::default();
        let dirty = Arc::new(AtomicBool::new(false));

        let writer_thread = {
            let dirty = dirty.clone();
            let writer = writer.clone();
            Some(thread::spawn(move || {
                use std::time::{Instant, Duration};

                const TICK: Duration = Duration::from_millis(1000);

                fn align(duration: Duration, tick: Duration) -> Duration {
                    let duration = duration.as_micros() as u64;
                    let tick = tick.as_micros() as u64;
                    Duration::from_micros(tick - (duration % tick))
                }

                loop {
                    let commit_begin = Instant::now();
                    if dirty.fetch_and(false, Ordering::SeqCst) {
                        writer.lock().unwrap().commit().unwrap();
                    }
                    let commit_end = Instant::now();
                    let elapsed = commit_end.duration_since(commit_begin);
                    thread::sleep(align(elapsed, TICK));
                }
            }))
        };

        LogIndexer { index, message_field, id_field, writer, queue, writer_thread, dirty }
    }

    pub fn write(&self, message: &str, id: u64) {
        use std::mem;

        match self.writer.try_lock() {
            Ok(writer) => {
                let (base, queue) = mem::replace(self.queue.lock().unwrap().deref_mut(), (0, Vec::new()));
                for (i, message) in queue.into_iter().enumerate() {
                    let mut doc = Document::default();
                    doc.add_text(self.message_field, message);
                    doc.add_u64(self.id_field, base + (i as u64));
                    writer.add_document(doc);
                }
                self.dirty.fetch_or(true, Ordering::SeqCst);
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

    pub fn read(&self, query: &str, limit: usize) -> impl Iterator<Item = (f32, u64)> {
        let reader = self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()
            .unwrap();
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.message_field]);
        let query = query_parser.parse_query(query).unwrap();
        let id_field = self.id_field;
        searcher
            .search(&query, &TopDocs::with_limit(limit))
            .unwrap()
            .into_iter()
            .filter_map(move |(score, doc_address)| {
                let retrieved_doc = searcher.doc(doc_address).unwrap();
                let f = retrieved_doc.field_values().iter()
                    .find(|x| x.field() == id_field)
                    .unwrap();
                match f.value() {
                    &schema::Value::U64(ref id) => Some((score, *id)),
                    _ => None,
                }
            })
    }
}
