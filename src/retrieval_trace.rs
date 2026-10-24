//! Opt-in, bounded retrieval diagnostics (009 T005). Evidence selection never
//! reads these records. Expected answers belong to the offline evaluator only.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use serde_json::Value;

const EVENT_BYTES: usize = 2 * 1024 * 1024;
const EVENT_COUNT: usize = 8192;
const RECORD_BYTES: usize = 4 * 1024 * 1024;
const RUN_BYTES: usize = 256 * 1024 * 1024;
const RUN_RECORDS: usize = 4096;

#[derive(Debug, Default, Serialize)]
struct Data {
    events: Vec<Value>,
    truncated: bool,
    #[serde(skip)]
    bytes: usize,
}

/// One request, shared by its bounded controls. No global/thread-local state.
#[derive(Clone, Debug, Default)]
pub struct RetrievalTrace(Arc<Mutex<Data>>);

impl RetrievalTrace {
    /// Call only at a real pipeline boundary. No source text is needed here;
    /// the final response is retained separately for offline packing analysis.
    pub fn record(&self, event: Value) {
        let mut data = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if data.truncated {
            return;
        }
        let mut count = crate::ByteCount(0);
        let bytes = match serde_json::to_writer(&mut count, &event) {
            Ok(()) => usize::try_from(count.0).unwrap_or(usize::MAX),
            Err(_) => EVENT_BYTES + 1,
        };
        if data.events.len() == EVENT_COUNT || data.bytes.saturating_add(bytes) > EVENT_BYTES {
            data.truncated = true;
            return;
        }
        data.bytes += bytes;
        data.events.push(event);
    }

    pub fn snapshot(&self) -> Value {
        let data = self.0.lock().unwrap_or_else(|e| e.into_inner());
        serde_json::json!({"events": data.events, "truncated": data.truncated})
    }
}

/// An operator-created private run directory. Refuses an existing path, including
/// symlinks; never appends to another run or overwrites an artifact. Both the number
/// of records and total persisted bytes are bounded for the owner's lifetime.
#[derive(Debug)]
pub struct TraceSink {
    directory: PathBuf,
    usage: Mutex<(usize, usize)>,
}

impl TraceSink {
    pub fn create(directory: &Path) -> std::io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory)?;
        Ok(Self {
            directory: directory.to_owned(),
            usage: Mutex::new((0, 0)),
        })
    }

    pub fn save(
        &self,
        operation: &str,
        trace: &RetrievalTrace,
        outcome: Value,
    ) -> std::io::Result<()> {
        let record = serde_json::json!({
            "version": 1,
            "operation": operation,
            "boundary": "foundry_delivery",
            "host_consumption": "unknown",
            "trace": trace.snapshot(),
            "outcome": outcome,
        });
        let bytes = serde_json::to_vec(&record)?;
        let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        if bytes.len() > RECORD_BYTES || usage.0 == RUN_RECORDS || usage.1 + bytes.len() > RUN_BYTES
        {
            return Err(std::io::Error::other("retrieval trace limit reached"));
        }
        // Reserve before writing: failed/partial writes consume their allowance,
        // too. Such files cannot pass an evaluator's complete-record check.
        usage.0 += 1;
        usage.1 += bytes.len();
        let path = self
            .directory
            .join(format!("{:04}-{}.json", usage.0, uuid::Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(&bytes)?;
        file.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_traces_are_explicitly_incomplete_and_runs_never_overwrite() {
        let trace = RetrievalTrace::default();
        for _ in 0..=EVENT_COUNT {
            trace.record(serde_json::json!({"stage":"candidate"}));
        }
        assert_eq!(
            trace.snapshot()["events"].as_array().unwrap().len(),
            EVENT_COUNT
        );
        assert_eq!(trace.snapshot()["truncated"], true);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("run");
        let sink = TraceSink::create(&path).unwrap();
        assert!(TraceSink::create(&path).is_err());
        sink.save("context", &trace, serde_json::json!({"status":"error"}))
            .unwrap();
        assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let file = fs::read_dir(&path).unwrap().next().unwrap().unwrap().path();
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
