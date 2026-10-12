//! One explicitly enabled configuration, advanced by the existing source watcher.
//! No model, new thread or durable queue: restart requires explicit enablement,
//! and each enablement reconciles the bounded admitted configuration.
use super::*;
use std::collections::VecDeque;

pub(crate) struct Maintenance {
    config: String,
    enabled: bool,
    completed: Option<u64>,
    revision: u64,
    config_hash: Option<String>,
    paths: VecDeque<String>,
    parsed: u64,
    reused: u64,
    error: Option<String>,
}
impl Maintenance {
    pub(crate) fn new(config: String) -> Self {
        Self {
            config,
            enabled: true,
            completed: None,
            revision: 0,
            config_hash: None,
            paths: VecDeque::new(),
            parsed: 0,
            reused: 0,
            error: None,
        }
    }
    pub(crate) fn pause(&mut self, config: &str) -> FResult<()> {
        if config != self.config {
            return Err(invalid("boundary configuration is not maintained"));
        }
        self.enabled = false;
        self.paths.clear();
        Ok(())
    }
    pub(crate) fn status(&self) -> serde_json::Value {
        serde_json::json!({"config":self.config,"enabled":self.enabled,
            "phase":if !self.enabled {"paused"} else if self.error.is_some() {"error"} else if self.paths.is_empty() && self.completed.is_some() {"waiting"} else {"maintaining"},
            "pending_paths":self.paths.len(),"completed_source_revision":self.completed,
            "parsed":self.parsed,"reused":self.reused,"last_error":self.error})
    }
    /// Returns whether this turn did maintenance. Source invalidation is already
    /// committed; errors remain visible here and never block source acceptance.
    pub(crate) fn step(&mut self, engine: &Engine, control: &Control) -> bool {
        if !self.enabled {
            return false;
        }
        let revision = match engine.source_revision() {
            Ok(r) => r,
            Err(_) => return false,
        };
        if self.completed == Some(revision) && self.paths.is_empty() {
            return false;
        }
        let result = self.advance(engine, revision, control);
        if result.is_err() {
            self.paths.clear();
            self.completed = Some(revision);
            // Avoid exposing arbitrary source or parser diagnostics in status.
            self.error = Some(
                "boundary maintenance failed; check indexed configuration and source availability"
                    .into(),
            );
        }
        true
    }
    fn advance(&mut self, engine: &Engine, revision: u64, control: &Control) -> FResult<()> {
        control.check()?;
        let current = engine.source(&self.config)?.ok_or(FoundryError::NotFound)?;
        if self.paths.is_empty() || self.config_hash.as_deref() != Some(&current.hash) {
            let tx = engine.db.begin_read()?;
            let (config, span) = config_in(&tx, &self.config)?;
            self.paths = config.paths().into_iter().collect();
            self.config_hash = Some(span.sha256);
            self.revision = revision;
            self.error = None;
        }
        if let Some(path) = self.paths.pop_front() {
            let report = engine.index_boundaries(&self.config, &[path], control)?;
            self.parsed = self.parsed.saturating_add(report.parsed as u64);
            self.reused = self.reused.saturating_add(report.reused as u64);
        }
        if self.paths.is_empty() {
            self.completed = Some(self.revision);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_turns_reuse_unchanged_files_and_resume_after_config_failure() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let e = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        let config = r#"{"v":1,"services":[{"id":"s","providers":["p.py"],"origins":["http://s"]}],"consumers":["c.py"]}"#;
        e.replace_source("boundary.json", config).unwrap();
        e.replace_source(
            "p.py",
            "from fastapi import FastAPI\napp = FastAPI()\n@app.get('/v1/x')\ndef x(): pass\n",
        )
        .unwrap();
        e.replace_source("c.py", "import httpx\nhttpx.get('http://s/v1/x')\n")
            .unwrap();
        let mut m = Maintenance::new("boundary.json".into());
        let control = Control::unbounded();
        assert!(m.step(&e, &control));
        assert_eq!(m.parsed, 1);
        assert!(m.step(&e, &control));
        assert_eq!(m.parsed, 2);
        assert!(!m.step(&e, &control));
        e.replace_source("c.py", "import httpx\nhttpx.get('http://s/v1/y')\n")
            .unwrap();
        for _ in 0..2 {
            assert!(m.step(&e, &control));
        }
        assert_eq!((m.parsed, m.reused), (3, 1));
        e.replace_source("boundary.json", "broken").unwrap();
        assert!(m.step(&e, &control));
        assert!(!m.step(&e, &control));
        assert_eq!(m.status()["phase"], "error");
        e.replace_source("boundary.json", config).unwrap();
        for _ in 0..2 {
            m.step(&e, &control);
        }
        assert_eq!(m.status()["phase"], "waiting");
        assert_eq!(
            m.parsed, 3,
            "identical restored config reuses accepted facts"
        );
        assert!(m.pause("other.json").is_err());
        m.pause("boundary.json").unwrap();
        e.delete_source("p.py").unwrap();
        assert!(!m.step(&e, &control));
        let mut restart = Maintenance::new("boundary.json".into());
        for _ in 0..2 {
            restart.step(&e, &control);
        }
        let links = e
            .boundary_relations("boundary.json", &["c.py".into()], &control)
            .unwrap();
        assert!(links.relations.iter().all(|r| r.resolution != "exact_key"));
    }
}
