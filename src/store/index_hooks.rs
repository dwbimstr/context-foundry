use std::cell::RefCell;
use std::sync::Arc;

/// One step of a page build, reported to [`Hooks::observer`].
#[derive(Debug)]
pub enum Event<'a> {
    /// A build thread starts the source at this path, inside the parse
    /// panic boundary: an observer that panics here is a parse panic.
    Build(&'a str),
    /// A build thread finished the source at this path.
    Built(&'a str),
    /// The source at `path` was handed out; `outstanding` bytes are now
    /// handed out and not yet consumed (their documents not yet added),
    /// this source's included.
    HandedOut { path: &'a str, outstanding: usize },
    /// The hand-out must make room: `outstanding` bytes plus the `next`
    /// source's would pass the bound, so the writer adds finished
    /// sources in key order first.
    Wait { outstanding: usize, next: usize },
    /// The writer adds one document, rendered as JSON, in add order.
    Add(&'a str),
}

pub type Observer = Arc<dyn Fn(&Event<'_>) + Send + Sync>;

#[derive(Clone, Default)]
pub struct Hooks {
    /// Build threads per page instead of min(available parallelism, 8).
    pub threads: Option<usize>,
    /// The hand-out bound instead of 64 MiB.
    pub handout_bytes: Option<usize>,
    pub observer: Option<Observer>,
}

thread_local! {
    static HOOKS: RefCell<Hooks> = const {
        RefCell::new(Hooks {
            threads: None,
            handout_bytes: None,
            observer: None,
        })
    };
}

/// Install `hooks` for the refreshes this thread runs.
pub fn install(hooks: Hooks) {
    HOOKS.with(|slot| *slot.borrow_mut() = hooks);
}

/// Remove this thread's hooks.
pub fn clear() {
    install(Hooks::default());
}

/// The live committed search documents of `path` in the derived index
/// on disk under `store_dir`, as sorted `(hash, start)` rows, read
/// through a fresh reader: a duplicate or a stale version is an extra
/// row.
pub fn committed_documents(store_dir: &std::path::Path, path: &str) -> Vec<(String, u64)> {
    use tantivy::schema::Value;
    let index =
        tantivy::Index::open_in_dir(store_dir.join("search")).expect("the derived index opens");
    let fields = super::fields_of(&index.schema());
    let reader: tantivy::IndexReader = index
        .reader_builder()
        .reload_policy(tantivy::ReloadPolicy::Manual)
        .try_into()
        .expect("a reader");
    let searcher = reader.searcher();
    let query = tantivy::query::TermQuery::new(
        tantivy::Term::from_field_text(fields.path, path),
        tantivy::schema::IndexRecordOption::Basic,
    );
    let mut documents: Vec<(String, u64)> = searcher
        .search(&query, &tantivy::collector::DocSetCollector)
        .expect("the path query runs")
        .into_iter()
        .map(|address| {
            let doc: tantivy::TantivyDocument = searcher.doc(address).expect("a stored doc");
            let hash = doc.get_first(fields.hash).and_then(|v| v.as_str());
            let start = doc.get_first(fields.start).and_then(|v| v.as_u64());
            (
                hash.unwrap_or_default().to_owned(),
                start.unwrap_or(u64::MAX),
            )
        })
        .collect();
    documents.sort();
    documents
}

impl Hooks {
    pub(super) fn current() -> Self {
        HOOKS.with(|slot| slot.borrow().clone())
    }

    pub(super) fn threads(&self) -> Option<usize> {
        self.threads
    }

    pub(super) fn handout_bytes(&self) -> Option<usize> {
        self.handout_bytes
    }

    pub(super) fn emit(&self, event: &Event<'_>) {
        if let Some(observer) = &self.observer {
            observer(event);
        }
    }
}
