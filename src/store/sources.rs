//! Source rows and their bookkeeping: the scan protocol (begin, mark seen,
//! sweep, finish), source writes and deletions, the pending queue and the
//! memory and brief drains. Indexing, search and the storage helpers stay in
//! the parent module.
use super::*;

impl Engine {
    pub fn source(&self, path: &str) -> FResult<Option<SourceMeta>> {
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        match sources.get(path)? {
            None => Ok(None),
            Some(v) => decode::<SourceMeta>(v.value(), "source").map(Some),
        }
    }

    /// Begin a scan: checked scan-id increment and `running` status, before
    /// any tree mutation.
    pub(crate) fn begin_scan(&self) -> FResult<u64> {
        self.begin_scan_kind(true)
    }

    /// Scoped watcher walks share seen IDs but do not claim a whole-root scan.
    pub(crate) fn begin_scan_kind(&self, full: bool) -> FResult<u64> {
        let tx = self.db.begin_write()?;
        let scan_id = {
            let mut meta = tx.open_table(META)?;
            // Required state is validated, never silently overwritten.
            read_scan_state(&meta)?;
            let current = read_counter(&meta, "scan_id")?;
            let next = current
                .checked_add(1)
                .ok_or(FoundryError::ScanIdExhausted)?;
            meta.insert("scan_id", next.to_string().as_str())?;
            if full {
                meta.insert("scan_status", "running")?;
            }
            next
        };
        tx.commit()?;
        Ok(scan_id)
    }

    pub(crate) fn current_scan_id(&self) -> FResult<u64> {
        read_counter(&self.db.begin_read()?.open_table(META)?, "scan_id")
    }

    pub(crate) fn finish_scan(&self, complete: bool) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            meta.insert(
                "scan_status",
                if complete { "complete" } else { "incomplete" },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Record a bounded page of encountered paths as seen in `scan_id` with one
    /// commit. Callers flush before the sweep, so the sweep reads a complete set.
    pub(crate) fn mark_seen_batch(&self, paths: &[String], scan_id: u64) -> FResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        fault!(SEEN_FLUSH, Some(self), None, &paths.len().to_string())?;
        let id = scan_id.to_string();
        let tx = self.db.begin_write()?;
        {
            let mut seen = tx.open_table(SEEN)?;
            for path in paths {
                seen.insert(path.as_str(), id.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Paged sweep of source rows not seen in `scan_id`. Each page commits
    /// atomically; an interruption retires only already-committed pages.
    pub(crate) fn sweep_unseen(
        &self,
        scan_id: u64,
        control: &crate::Control,
    ) -> FResult<(u64, bool)> {
        let mut deleted = 0;
        let mut after = None;
        loop {
            if control.check().is_err() {
                return Ok((deleted, true));
            }
            let (count, next) = self.sweep_unseen_page(scan_id, None, after.as_deref(), control)?;
            deleted += count;
            let Some(next) = next else {
                return Ok((deleted, false));
            };
            after = Some(next);
        }
    }

    /// One bounded deletion transaction, optionally restricted to one path/subtree.
    /// The caller must have completed its walk without failures before the first page.
    pub(crate) fn sweep_unseen_page(
        &self,
        scan_id: u64,
        scope: Option<&str>,
        after: Option<&str>,
        _control: &crate::Control,
    ) -> FResult<(u64, Option<String>)> {
        let page: Vec<String> = {
            let tx = self.db.begin_read()?;
            let sources = tx.open_table(SOURCES)?;
            let mut page = Vec::new();
            let prefix = scope.map(|path| format!("{path}/"));
            if after.is_none()
                && let Some(path) = scope
                && sources.get(path)?.is_some()
            {
                page.push(path.to_owned());
            }
            let start = match (after, prefix.as_deref()) {
                (Some(key), Some(prefix)) if key < prefix => Bound::Included(prefix),
                (Some(key), _) => Bound::Excluded(key),
                (None, Some(prefix)) => Bound::Included(prefix),
                (None, None) => Bound::Unbounded,
            };
            for row in sources
                .range::<&str>((start, Bound::Unbounded))?
                .take(PAGE - page.len())
            {
                let (key, _) = row?;
                if prefix
                    .as_deref()
                    .is_some_and(|p| !key.value().starts_with(p))
                {
                    break;
                }
                page.push(key.value().to_owned());
            }
            page
        };
        if page.is_empty() {
            return Ok((0, None));
        }
        fault!(
            SWEEP_PAGE,
            Some(self),
            Some(_control),
            &page.len().to_string()
        )?;
        let mut retired = 0u64;
        let mut changed_paths = Vec::new();
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let mut stored = tx.open_table(CHUNKS)?;
            let mut pending = tx.open_table(PENDING)?;
            let mut seen = tx.open_table(SEEN)?;
            let mut meta = tx.open_table(META)?;
            for path in &page {
                let seen_now = seen
                    .get(path.as_str())?
                    .and_then(|v| v.value().parse::<u64>().ok());
                if seen_now != Some(scan_id)
                    && let Some(old) = sources.remove(path.as_str())?
                {
                    let old: SourceMeta = decode(old.value(), "source")?;
                    for i in 0..old.chunks {
                        stored.remove(chunk_key(path, i).as_str())?;
                    }
                    pending.insert(source_pending_key(path).as_str(), "deleted")?;
                    seen.remove(path.as_str())?;
                    retired += 1;
                    changed_paths.push(path.as_str());
                    let revision = read_counter(&meta, "source_revision")?
                        .checked_add(retired)
                        .ok_or(FoundryError::RevisionExhausted)?;
                    crate::neural::work::changed(&tx, path, None, revision)?;
                }
            }
            if retired > 0 {
                let revision = read_counter(&meta, "source_revision")?;
                let next = revision
                    .checked_add(retired)
                    .ok_or(FoundryError::RevisionExhausted)?;
                meta.insert("source_revision", next.to_string().as_str())?;
            }
        }
        if changed_paths.is_empty() {
            tx.commit()?;
        } else {
            self.semantic_commit(tx, changed_paths, std::iter::empty(), false)?;
        }
        if retired > 0 {
            self.semantic_wake.notify();
        }
        Ok((retired, page.last().cloned()))
    }

    /// All authoritative source/chunk changes, pending index work and the
    /// source revision commit together. Acknowledgement follows commit.
    pub fn replace_source(&self, path: &str, content: &str) -> FResult<bool> {
        let hash = admitted_source_hash(path, content)?;
        let tx = self.db.begin_write()?;
        if !Self::stage_source(&tx, path, content, &hash)? {
            return Ok(false);
        }
        // A failure here drops the transaction: prior source bytes stay intact.
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        self.semantic_commit(tx, [path], std::iter::empty(), false)?;
        self.semantic_wake.notify();
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    /// Ordered, atomic ingestion batch. No partial success within a batch; source
    /// revisions still advance once per changed path. The watcher uses singles.
    pub(crate) fn replace_prepared_sources(
        &self,
        sources: &[PreparedSource],
        control: &crate::Control,
    ) -> FResult<usize> {
        if sources.len() > 4 {
            return Err(FoundryError::InvalidArgument(
                "source commit batch exceeds four files".into(),
            ));
        }
        control.check()?;
        let tx = self.db.begin_write()?;
        let mut changed = Vec::new();
        for source in sources {
            control.check()?;
            if Self::stage_source(&tx, &source.path, &source.body, &source.hash)? {
                changed.push(source.path.as_str());
            }
        }
        if changed.is_empty() {
            return Ok(0);
        }
        #[cfg(feature = "test-faults")]
        for path in &changed {
            fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        }
        fault!(
            SOURCE_BATCH_BEFORE_COMMIT,
            Some(self),
            Some(control),
            &changed.len().to_string()
        )?;
        control.check()?;
        let count = changed.len();
        self.semantic_commit(tx, changed.iter().copied(), std::iter::empty(), false)?;
        self.semantic_wake.notify();
        #[cfg(feature = "test-faults")]
        for path in &changed {
            fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        }
        Ok(count)
    }

    fn stage_source(tx: &WriteTransaction, path: &str, content: &str, hash: &str) -> FResult<bool> {
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.get(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => None,
            };
            if old.as_ref().is_some_and(|old| old.hash == hash) {
                return Ok(false);
            }
            let mut stored = tx.open_table(CHUNKS)?;
            if let Some(old) = &old {
                for i in 0..old.chunks {
                    stored.remove(chunk_key(path, i).as_str())?;
                }
            }
            let mut count = 0;
            for (i, chunk) in chunks(path, hash, content).enumerate() {
                stored.insert(
                    chunk_key(path, i).as_str(),
                    serde_json::to_string(&chunk)?.as_str(),
                )?;
                count = i + 1;
            }
            sources.insert(
                path,
                serde_json::to_string(&SourceMeta {
                    hash: hash.to_owned(),
                    chunks: count,
                    bytes: content.len(),
                    lines: content.lines().count(),
                })?
                .as_str(),
            )?;
            tx.open_table(PENDING)?
                .insert(source_pending_key(path).as_str(), hash)?;
            bump_revision(tx, 1)?;
            crate::neural::work::changed(tx, path, Some(hash), revision_in(tx)?)?;
        }
        Ok(true)
    }

    pub fn delete_source(&self, path: &str) -> FResult<bool> {
        validate_path(path)?;
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.remove(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => return Ok(false),
            };
            let Some(old) = old else {
                return Ok(false);
            };
            let mut stored = tx.open_table(CHUNKS)?;
            for i in 0..old.chunks {
                stored.remove(chunk_key(path, i).as_str())?;
            }
            tx.open_table(PENDING)?
                .insert(source_pending_key(path).as_str(), "deleted")?;
            bump_revision(&tx, 1)?;
            crate::neural::work::changed(&tx, path, None, revision_in(&tx)?)?;
        }
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        self.semantic_commit(tx, [path], std::iter::empty(), false)?;
        self.semantic_wake.notify();
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    /// Pending `source:` work only (008): scan and multi-root readers report
    /// their own namespace; readiness and drains keep the total.
    pub fn pending_source_work(&self) -> FResult<u64> {
        let tx = self.db.begin_read()?;
        count_pending_prefix(&tx, "source:")
    }

    pub fn pending(&self) -> FResult<u64> {
        Ok(self.db.begin_read()?.open_table(PENDING)?.len()?)
    }

    pub fn source_revision(&self) -> FResult<u64> {
        let tx = self.db.begin_read()?;
        read_counter(&tx.open_table(META)?, "source_revision")
    }

    /// Best-effort: apply the one `memory:<id>` pending key to the derived
    /// index right after its mutation committed, through the same document
    /// and clear-if-unchanged rules as the normal drain. Any failure (broken
    /// or missing index, I/O) leaves the key pending and is not reported:
    /// the authoritative mutation already succeeded and replay converges.
    pub(crate) fn drain_memory_key(&mut self, id: &str) {
        let _ = self.try_drain_record_key(id, "memory");
    }

    pub(crate) fn drain_brief_key(&mut self, id: &str) {
        let _ = self.try_drain_record_key(id, "brief");
    }
    fn try_drain_record_key(&mut self, id: &str, kind: &str) -> FResult<()> {
        if self.repair_reason.is_some() {
            return Ok(());
        }
        let key = format!("{kind}:{id}");
        let (value, record) = {
            let tx = self.db.begin_read()?;
            let Some(value) = tx
                .open_table(PENDING)?
                .get(key.as_str())?
                .map(|v| v.value().to_owned())
            else {
                return Ok(());
            };
            let record = if kind == "brief" {
                tx.open_table(BRIEFS)?
                    .get(id)?
                    .and_then(|raw| crate::brief::decode_row(id, raw.value()).ok())
                    .map(|r| (r.id, r.revision, r.brief.searchable_text()))
            } else {
                tx.open_table(MEMORY)?
                    .get(id)?
                    .and_then(|raw| serde_json::from_str::<MemoryRecord>(raw.value()).ok())
                    .map(|r| (r.id, r.revision, r.text))
            };
            (value, record)
        };
        let Some(handles) = self.search.as_mut() else {
            return Ok(());
        };
        handles
            .writer
            .delete_term(Term::from_field_text(handles.fields.key, &key));
        if let Some(record) = &record {
            handles.writer.add_document(record_document(
                &handles.fields,
                kind,
                &record.0,
                record.1,
                &record.2,
            ))?;
        }
        handles.writer.commit()?;
        handles.reader.reload()?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PENDING)?;
            let unchanged = table.get(key.as_str())?.is_some_and(|v| v.value() == value);
            if unchanged {
                table.remove(key.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
