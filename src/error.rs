//! Typed store/retrieval errors with contract codes. One error type serves CLI,
//! library and MCP consumers so no boundary invents a second naming scheme.
use crate::response::FinalRender;
use serde::Serialize;
use std::fmt;

pub type FResult<T> = Result<T, FoundryError>;

/// Named failures from the 001/003 contracts. `Internal` wraps unexpected
/// library/OS errors; everything else is a contract code.
#[derive(Debug)]
pub enum FoundryError {
    InvalidArgument(String),
    UnsupportedMode(String),
    WrongWorkspace,
    WorkspaceUnbound,
    NotFound,
    StaleHandle,
    InvalidRange,
    CorruptSource(String),
    BudgetTooSmall {
        minimum_tokens: usize,
    },
    Cancelled(Option<PartialIndexCounts>),
    DeadlineExceeded(Option<PartialIndexCounts>),
    /// Indexing finished incomplete for a non-cancellation reason; carries
    /// committed counts so no partial read is reported as success.
    IndexIncomplete(PartialIndexCounts),
    StoreNotFound,
    UnrecognizedStore(String),
    StoreBusy,
    UnsupportedSchema {
        found: String,
    },
    UpgradeRequired {
        found: String,
    },
    CorruptStore(String),
    RepairRequired(String),
    RepairPathConflict(String),
    ScanIdExhausted,
    RevisionExhausted,
    PlatformUnsupported(String),
    /// A source path or the workspace root was replaced by a symlink or a
    /// non-directory/non-regular object before it could be opened.
    UnsafeSourcePath(String),
    /// Permission denied on the root or one of its components.
    PermissionDenied(String),
    /// An optional graph record cannot be decoded. Component-local: context
    /// degrades to source evidence; a direct graph request names this code.
    GraphInvalid(String),
    /// A memory mutation collides with the live record's revision or fields
    /// (008): nothing was written and no revision was consumed.
    Conflict(String),
    /// A memory forget names an ID this store does not hold (008). No write
    /// happened; the surface renders it as a content-free outcome report.
    AlreadyAbsent(String),
    /// A memory record cannot be decoded (008). Names the record; source
    /// operations remain available.
    CorruptMemory(String),
    /// A named 005 compiler-graph failure (SCIP import or `references`):
    /// `unbound_artifact`, `stale_artifact`, `invalid_range`,
    /// `unsupported_encoding`, `artifact_too_large`, `manifest_too_large`,
    /// `document_too_large`, `producer_incomplete`, `scratch_full`,
    /// `duplicate_document`, `duplicate_input`, `symbol_not_found`,
    /// `ambiguous_symbol`. Callers build it through `crate::scip::fail`.
    Scip {
        code: &'static str,
        message: String,
    },
    /// A caller-named import file is missing, not a regular file or
    /// unreadable. An invalid invocation (CLI exit 2).
    ArtifactUnavailable(String),
    /// A named 009 semantic-retrieval failure: `semantic_unavailable`,
    /// `profile_invalid`, `isolation_unavailable`, `cache_full`,
    /// `budget_exhausted`, `cache_corrupt` or a provider code carried from
    /// [`crate::neural::provider::ProviderError`] (`provider_busy`,
    /// `provider_timeout`, `provider_exited`, `provider_malformed`,
    /// `resource_limit`, `input_too_large`). Source access stays available.
    Semantic {
        code: &'static str,
        message: String,
    },
    Internal(anyhow::Error),
}
/// Counts-only partial index state for bounded MCP/CLI cancellation errors.
/// No samples: large diagnostics belong to full CLI reports.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PartialIndexCounts {
    pub changed: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub excluded: u64,
    pub failed: u64,
    pub pending_sources: u64,
    pub scan_complete: bool,
    pub deletions_deferred: bool,
}

impl FoundryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument(_) => "invalid_argument",
            Self::UnsupportedMode(_) => "unsupported_mode",
            Self::WrongWorkspace => "wrong_workspace",
            Self::WorkspaceUnbound => "workspace_unbound",
            Self::NotFound => "not_found",
            Self::StaleHandle => "stale_handle",
            Self::InvalidRange => "invalid_range",
            Self::CorruptSource(_) => "corrupt_source",
            Self::BudgetTooSmall { .. } => "budget_too_small",
            Self::StoreNotFound => "store_not_found",
            Self::UnrecognizedStore(_) => "unrecognized_store",
            Self::StoreBusy => "store_busy",
            Self::UnsupportedSchema { .. } => "unsupported_schema",
            Self::UpgradeRequired { .. } => "upgrade_required",
            Self::CorruptStore(_) => "corrupt_store",
            Self::RepairRequired(_) => "repair_required",
            Self::RepairPathConflict(_) => "repair_path_conflict",
            Self::ScanIdExhausted => "scan_id_exhausted",
            Self::RevisionExhausted => "revision_exhausted",
            Self::Cancelled(_) => "cancelled",
            Self::DeadlineExceeded(_) => "deadline_exceeded",
            Self::IndexIncomplete(_) => "index_incomplete",
            Self::PlatformUnsupported(_) => "platform_unsupported",
            Self::UnsafeSourcePath(_) => "unsafe_source_path",
            Self::PermissionDenied(_) => "permission_denied",
            Self::GraphInvalid(_) => "graph_invalid",
            Self::Conflict(_) => "conflict",
            Self::AlreadyAbsent(_) => "already_absent",
            Self::CorruptMemory(_) => "corrupt_memory",
            Self::Scip { code, .. } => code,
            Self::ArtifactUnavailable(_) => "artifact_unavailable",
            Self::Semantic { code, .. } => code,
            Self::Internal(_) => "internal",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::StoreBusy
                | Self::Cancelled(_)
                | Self::DeadlineExceeded(_)
                | Self::RepairRequired(_)
        ) || matches!(self, Self::Semantic { code, .. } if *code == "provider_busy")
    }

    /// CLI process exit codes: 2 invalid argument/unsupported version or mode,
    /// 3 store busy, 130 cooperative cancellation, 1 other runtime failures.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_)
            | Self::UnsupportedMode(_)
            | Self::InvalidRange
            | Self::ArtifactUnavailable(_)
            | Self::UnsupportedSchema { .. }
            | Self::UpgradeRequired { .. } => 2,
            Self::StoreBusy => 3,
            Self::Cancelled(_) | Self::DeadlineExceeded(_) => 130,
            _ => 1,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidArgument(m) => format!("invalid argument: {m}"),
            Self::UnsupportedMode(m) => format!("unsupported mode: {m}"),
            Self::WrongWorkspace => "handle belongs to a different workspace".into(),
            Self::WorkspaceUnbound => "store is not bound to a workspace".into(),
            Self::NotFound => "source not found".into(),
            Self::StaleHandle => "source hash changed since the handle was issued".into(),
            Self::InvalidRange => {
                "byte range is outside the source or not on UTF-8 boundaries".into()
            }
            Self::CorruptSource(m) => format!("stored source chunks are inconsistent: {m}"),
            Self::BudgetTooSmall { minimum_tokens } => {
                format!("token budget cannot fit the response envelope; minimum {minimum_tokens}")
            }
            Self::StoreNotFound => "store does not exist; index explicitly to initialize".into(),
            Self::UnrecognizedStore(m) => format!("directory is not a recognized store: {m}"),
            Self::StoreBusy => "store is locked by another owner".into(),
            Self::UnsupportedSchema { found } => {
                format!("unsupported store schema {found}; no automatic upgrade")
            }
            Self::UpgradeRequired { found } => {
                format!("store schema {found} requires an explicit upgrade-store --to 5")
            }
            Self::CorruptStore(m) => format!("authoritative store data is corrupt: {m}"),
            Self::RepairRequired(m) => format!("derived search index needs explicit repair: {m}"),
            Self::RepairPathConflict(m) => format!("repair refuses unrecognized path: {m}"),
            Self::ScanIdExhausted => "scan counter exhausted".into(),
            Self::RevisionExhausted => "source revision counter exhausted".into(),
            Self::Cancelled(_) => "operation cancelled; committed counts attached".into(),
            Self::DeadlineExceeded(_) => {
                "operation deadline exceeded; committed counts attached".into()
            }
            Self::IndexIncomplete(_) => "indexing incomplete; committed counts attached".into(),
            Self::PlatformUnsupported(m) => format!("platform limitation: {m}"),
            Self::UnsafeSourcePath(m) => format!("unsafe source path: {m}"),
            Self::PermissionDenied(m) => format!("path access denied: {m}"),
            Self::GraphInvalid(m) => format!("graph record is invalid: {m}"),
            Self::Conflict(m) => {
                format!("memory record conflicts with the live revision or fields: {m}")
            }
            Self::AlreadyAbsent(m) => format!("memory record {m} is absent"),
            Self::CorruptMemory(m) => format!("memory record cannot be decoded: {m}"),
            Self::Scip { message, .. } => message.clone(),
            Self::ArtifactUnavailable(m) => format!("import file is unavailable: {m}"),
            Self::Semantic { code, message } => format!("semantic retrieval ({code}): {message}"),
            Self::Internal(e) => format!("{e:#}"),
        }
    }

    /// Bounded `{"code","message","retryable"}` error JSON for stderr/CLI
    /// (<= 1024 bytes). Identity rendering of [`Self::bounded_rendered`].
    pub fn bounded_json(&self) -> String {
        self.bounded_rendered(&|application| application.to_owned())
    }

    /// The exact emitted error bytes at the final boundary. `render` maps the
    /// compact application error JSON to what the transport emits (MCP wraps
    /// it in an escaped tool-result block); the 1024-byte cap applies to that
    /// final rendering, not to the inner JSON. Always terminates: every retry
    /// strictly shortens the message, and an empty message is the floor.
    /// Source bodies never enter errors.
    pub fn bounded_rendered(&self, render: FinalRender) -> String {
        const CAP: usize = 1024;
        let partial = match self {
            Self::Cancelled(p) | Self::DeadlineExceeded(p) => p
                .as_ref()
                .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
            Self::IndexIncomplete(c) => {
                Some(serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
            }
            _ => None,
        };
        let mut message = self.message();
        if matches!(
            self,
            Self::Cancelled(_) | Self::DeadlineExceeded(_) | Self::IndexIncomplete(_)
        ) {
            // Fixed ASCII message for index interruption codes.
            message = message.chars().filter(char::is_ascii).take(256).collect();
        }
        truncate_on_boundary(&mut message, 512);
        let build = |message: &str| {
            let mut value = serde_json::json!({
                "code": self.code(),
                "message": message,
                "retryable": self.retryable(),
            });
            if let (Some(partial), Some(object)) = (&partial, value.as_object_mut()) {
                object.insert("partial".into(), partial.clone());
            }
            render(&serde_json::to_string(&value).unwrap_or_default())
        };
        let mut emitted = build(&message);
        while emitted.len() > CAP && !message.is_empty() {
            let keep = message.len() / 2;
            truncate_on_boundary(&mut message, keep);
            emitted = build(&message);
        }
        emitted
    }
}

/// Shorten `text` to at most `max` bytes on a char boundary; when it had to
/// cut, the result ends with an ellipsis and still fits `max`.
fn truncate_on_boundary(text: &mut String, max: usize) {
    const ELLIPSIS: char = '\u{2026}';
    if text.len() <= max {
        return;
    }
    let mut cut = max.saturating_sub(ELLIPSIS.len_utf8());
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text.truncate(cut);
    if max >= ELLIPSIS.len_utf8() {
        text.push(ELLIPSIS);
    }
}

impl fmt::Display for FoundryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for FoundryError {}

impl From<anyhow::Error> for FoundryError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

impl From<std::io::Error> for FoundryError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(anyhow::anyhow!(e))
    }
}

impl From<serde_json::Error> for FoundryError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(anyhow::anyhow!(e))
    }
}

/// 009 provider failures cross into the shared error type under their own
/// contract codes; nothing is collapsed into `internal`.
impl From<crate::neural::provider::ProviderError> for FoundryError {
    fn from(e: crate::neural::provider::ProviderError) -> Self {
        Self::Semantic {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

/// redb 4.3 splits errors across StorageError/TableError/TransactionError/
/// CommitError/DatabaseError. Map each to the correct named contract code:
/// lock contention is `store_busy`, real corruption is `corrupt_store`,
/// everything else is ordinary runtime failure. Never collapse busy and
/// corruption into one code.
impl From<redb::StorageError> for FoundryError {
    fn from(e: redb::StorageError) -> Self {
        match e {
            redb::StorageError::Corrupted(m) => Self::CorruptStore(m),
            redb::StorageError::LockPoisoned(_) | redb::StorageError::PreviousIo => {
                Self::CorruptStore("database needs to be closed and reopened".into())
            }
            redb::StorageError::ValueTooLarge(n) => {
                Self::InvalidArgument(format!("value exceeds the storage limit ({n} bytes)"))
            }
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<redb::TableError> for FoundryError {
    fn from(e: redb::TableError) -> Self {
        match e {
            redb::TableError::Storage(storage) => storage.into(),
            redb::TableError::TableDoesNotExist(name) => {
                Self::CorruptStore(format!("table {name} missing from the store"))
            }
            redb::TableError::TableTypeMismatch { table, .. } => {
                Self::CorruptStore(format!("table {table} has an incompatible type"))
            }
            redb::TableError::TypeDefinitionChanged { name, .. } => {
                Self::CorruptStore(format!("table type definition for {name:?} changed"))
            }
            redb::TableError::TableExists(name) | redb::TableError::TableAlreadyOpen(name, _) => {
                Self::Internal(anyhow::anyhow!("table conflict on {name}"))
            }
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<redb::TransactionError> for FoundryError {
    fn from(e: redb::TransactionError) -> Self {
        match e {
            redb::TransactionError::Storage(storage) => storage.into(),
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<redb::CommitError> for FoundryError {
    fn from(e: redb::CommitError) -> Self {
        match e {
            redb::CommitError::TransactionPoisoned => {
                Self::CorruptStore("transaction was poisoned; reopen the store".into())
            }
            redb::CommitError::Storage(storage) => storage.into(),
            _ => Self::Internal(anyhow::anyhow!("commit failed")),
        }
    }
}

impl From<redb::DatabaseError> for FoundryError {
    fn from(e: redb::DatabaseError) -> Self {
        match e {
            redb::DatabaseError::DatabaseAlreadyOpen => Self::StoreBusy,
            redb::DatabaseError::UpgradeRequired(actual) => Self::CorruptStore(format!(
                "redb file format {actual} requires a manual upgrade"
            )),
            redb::DatabaseError::Storage(storage) => storage.into(),
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<redb::Error> for FoundryError {
    fn from(e: redb::Error) -> Self {
        match e {
            redb::Error::DatabaseAlreadyOpen => Self::StoreBusy,
            redb::Error::Corrupted(m) => Self::CorruptStore(m),
            redb::Error::UpgradeRequired(actual) => Self::CorruptStore(format!(
                "redb file format {actual} requires a manual upgrade"
            )),
            redb::Error::TransactionPoisoned | redb::Error::PreviousIo => {
                Self::CorruptStore("database needs to be closed and reopened".into())
            }
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<tantivy::TantivyError> for FoundryError {
    fn from(e: tantivy::TantivyError) -> Self {
        use tantivy::TantivyError as E;
        match e {
            E::OpenDirectoryError(_)
            | E::OpenReadError(_)
            | E::OpenWriteError(_)
            | E::DataCorruption(_)
            | E::IndexAlreadyExists => Self::CorruptStore(format!("derived index error: {e}")),
            E::SchemaError(e) => Self::CorruptStore(format!("derived index schema mismatch: {e}")),
            other => Self::Internal(anyhow::anyhow!(other)),
        }
    }
}

impl From<std::num::ParseIntError> for FoundryError {
    fn from(e: std::num::ParseIntError) -> Self {
        Self::Internal(anyhow::anyhow!(e))
    }
}
