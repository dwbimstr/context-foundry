//! Store lifecycle: initialize, open, schema upgrade, status and the explicit
//! derived-index repair. Workspace binding, sources, indexing and search stay
//! in the parent module.
use super::*;

impl Engine {
    /// Explicit initializer: accepts a missing or empty directory, refuses a
    /// nonempty directory without a recognized store, binds the workspace
    /// root and creates the empty derived index.
    pub fn initialize(store_dir: &Path, workspace_root: &Path) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        // Validate arguments/root before initializing anything: invalid index
        // arguments — or a root that cannot be held as a descriptor — must not
        // create a store at all.
        let root = workspace_root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        if !root.is_dir() {
            return Err(FoundryError::InvalidArgument(
                "workspace root must be a directory".into(),
            ));
        }
        // A not-yet-existing store directory cannot equal an existing root.
        if store_dir
            .canonicalize()
            .is_ok_and(|existing_store| existing_store == root)
        {
            return Err(FoundryError::InvalidArgument(
                "store directory cannot be the workspace root".into(),
            ));
        }
        crate::ingest::ensure_holdable_root(&root)?;
        if store_dir.exists() {
            let nonempty = std::fs::read_dir(store_dir)
                .map_err(FoundryError::from)?
                .any(|_| true);
            if nonempty && !db_path.exists() {
                return Err(FoundryError::UnrecognizedStore(
                    store_dir.display().to_string(),
                ));
            }
        } else {
            std::fs::create_dir_all(store_dir)?;
        }
        let db = Database::create(&db_path)?;
        let tx = db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(version) = meta.get("schema")? {
                return Err(FoundryError::InvalidArgument(format!(
                    "store already initialized with schema {}",
                    version.value()
                )));
            }
            initialize_tables(&tx)?;
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
            meta.insert("source_revision", "0")?;
            meta.insert("scan_id", "0")?;
            meta.insert("scan_status", "never")?;
            meta.insert("memory_revision", "0")?;
            meta.insert("task_revision", "0")?;
            meta.insert("brief_revision", "0")?;
            meta.insert("search_schema", SEARCH_SCHEMA)?;
            let root_str = root.to_str().ok_or_else(|| {
                FoundryError::InvalidArgument("workspace path is not UTF-8".into())
            })?;
            meta.insert("workspace", root_str)?;
            meta.insert("workspace_id", crate::digest(root_str.as_bytes()).as_str())?;
        }
        tx.commit()?;
        std::fs::create_dir_all(store_dir.join("search"))?;
        let index = Index::create_in_dir(store_dir.join("search"), search_schema())?;
        let handles = search_handles(index)?;
        // The canonical store path, computed ONCE at bind time; the
        // descriptor below and the stored display path both come from it and
        // neither is ever resolved again for semantic work.
        let canonical = store_dir.canonicalize()?;
        // Bind the store-directory descriptor once, after the database (and
        // its lock) is held: every later semantic operation works through a
        // duplicate of it, never through the pathname.
        let semantic_dir = crate::neural::anchor::Dir::open_path(&canonical)
            .map_err(|e| crate::neural::anchor::conflict_or_io(e, "store directory"))?;
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        Ok(Self {
            db,
            directory: canonical,
            search: Some(handles),
            repair_reason: None,
            workspace,
            workspace_id,
            schema: SCHEMA_VERSION,
            parse_panics: ParseFailures::default(),
            index_pool: None,
            semantic_dir,
            #[cfg(feature = "semantic")]
            semantic_segments: Default::default(),
            #[cfg(feature = "semantic")]
            semantic_mapping: Default::default(),
            semantic_wake: Default::default(),
        })
    }

    fn read_binding(db: &Database) -> FResult<(Option<String>, Option<String>)> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        Ok((
            meta.get("workspace")?.map(|v| v.value().to_owned()),
            meta.get("workspace_id")?.map(|v| v.value().to_owned()),
        ))
    }

    /// Open existing state only. Never creates files, tables or directories,
    /// never upgrades schema. Derived-index trouble degrades search to
    /// `repair_required`; authoritative reads stay available.
    pub fn open_existing(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, true, false)
    }

    /// Authoritative-only open used by repair: no Tantivy construction first.
    pub(crate) fn open_authoritative(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, false, false)
    }

    fn open_with(store_dir: &Path, construct_search: bool, repair_mapping: bool) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        let schema = {
            let tx = db.begin_read()?;
            let meta = tx
                .open_table(META)
                .map_err(|e| FoundryError::CorruptStore(format!("meta table unreadable: {e}")))?;
            let Some(version) = meta.get("schema")? else {
                return Err(FoundryError::UnrecognizedStore(
                    "store has no schema marker".into(),
                ));
            };
            match version.value() {
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" => {
                    return Err(FoundryError::UpgradeRequired {
                        found: version.value().to_owned(),
                    });
                }
                "12" => 12u32,
                other => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: other.to_owned(),
                    });
                }
            }
        };
        // Confirm the schema-12 tables exist; missing authoritative tables in
        // a schema-12 store are corruption, not something an open recreates.
        {
            let tx = db.begin_read()?;
            tx.open_table(SEEN).map_err(|e| {
                FoundryError::CorruptStore(format!("scan_seen table unreadable: {e}"))
            })?;
            tx.open_table(SOURCES)
                .map_err(|e| FoundryError::CorruptStore(format!("sources table: {e}")))?;
            tx.open_table(CHUNKS)
                .map_err(|e| FoundryError::CorruptStore(format!("chunks table: {e}")))?;
            tx.open_table(PENDING)
                .map_err(|e| FoundryError::CorruptStore(format!("pending table: {e}")))?;
            tx.open_table(MEMORY)
                .map_err(|e| FoundryError::CorruptStore(format!("memory table: {e}")))?;
            tx.open_table(crate::task_context::TASKS)
                .map_err(|e| FoundryError::CorruptStore(format!("task_context table: {e}")))?;
            read_counter(&tx.open_table(META)?, "task_revision")?;
            tx.open_table(BRIEFS)
                .map_err(|e| FoundryError::CorruptStore(format!("source_briefs table: {e}")))?;
            read_counter(&tx.open_table(META)?, "brief_revision")?;
            crate::graph::check_compiler_tables(&tx)?;
            crate::boundary::check(&tx)?;
            crate::neural::cache::check_tables(&db)?;
            crate::neural::work::check(&db)?;
            if !repair_mapping {
                crate::neural::mapping_store::check(&tx)?;
            }
            crate::knowledge::cache::check(&db)?;
            crate::learning::check_tables(&db)?;
        }
        if repair_mapping {
            // Only explicit repair replaces this disposable lookup. Ordinary
            // authoritative reads and migrations never silently repair it.
            let tx = db.begin_write()?;
            crate::neural::mapping_store::purge(&tx)?;
            tx.commit()?;
        }
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        let marker = Self::read_marker(&db)?;
        let mut repair_reason = marker
            .is_some()
            .then(|| "search_rebuild_required marker is set".to_owned());
        let mut search = None;
        if marker.is_none() && construct_search {
            // Index version gate: anything but the current format, recorded
            // or actual, needs an explicit repair; an open never rebuilds.
            let recorded = {
                let tx = db.begin_read()?;
                let meta = tx.open_table(META)?;
                meta.get("search_schema")?
                    .map(|v| v.value() == SEARCH_SCHEMA)
            };
            if recorded != Some(true) {
                repair_reason = Some(SEARCH_SCHEMA_REASON.to_owned());
            } else {
                match open_search(store_dir) {
                    Ok(handles) => search = Some(handles),
                    Err(reason) => repair_reason = Some(reason),
                }
            }
        }
        // Bind the store-directory descriptor once, after the database (and
        // its lock) is held. The path is canonicalized here — ONCE, at bind
        // time — and never resolved again: every later semantic operation
        // works through a duplicate of this descriptor, so an ancestor
        // renamed or substituted after the bind cannot redirect it.
        let canonical = store_dir.canonicalize()?;
        let semantic_dir = crate::neural::anchor::Dir::open_path(&canonical)
            .map_err(|e| crate::neural::anchor::conflict_or_io(e, "store directory"))?;
        Ok(Self {
            db,
            directory: canonical,
            search,
            repair_reason,
            workspace,
            workspace_id,
            schema,
            parse_panics: ParseFailures::default(),
            index_pool: None,
            semantic_dir,
            #[cfg(feature = "semantic")]
            semantic_segments: Default::default(),
            #[cfg(feature = "semantic")]
            semantic_mapping: Default::default(),
            semantic_wake: Default::default(),
        })
    }

    fn read_marker(db: &Database) -> FResult<Option<RebuildMarker>> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        match meta.get("search_rebuild_required")? {
            None => Ok(None),
            Some(raw) => decode::<RebuildMarker>(raw.value(), "rebuild marker")
                .map(Some)
                .map_err(|_| FoundryError::CorruptStore("rebuild marker cannot be decoded".into())),
        }
    }

    /// Explicit v1..=v11 -> v12 transaction under exclusive ownership. The
    /// cumulative steps preserve source, memory, graph, semantic and learning
    /// data. Version 8 adds source_briefs and its never-reset brief_revision.
    /// Version 10 adds atomic knowledge completion caching and brief lineage.
    /// Publish the schema last; interrupted upgrades remain wholly old or new.
    /// Only the current version is a target. Older readers must refuse it.
    pub fn upgrade_store(store_dir: &Path, to: u32, control: &crate::Control) -> FResult<()> {
        if to != SCHEMA_VERSION {
            return Err(FoundryError::UnsupportedMode(format!(
                "only upgrade-store --to {SCHEMA_VERSION} is supported"
            )));
        }
        control.check()?;
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        let from = {
            let tx = db.begin_read()?;
            let meta = tx.open_table(META)?;
            match meta.get("schema")? {
                None => {
                    return Err(FoundryError::UnrecognizedStore(
                        "store has no schema marker".into(),
                    ));
                }
                Some(v) if v.value() == "12" => return Ok(()), // already upgraded
                Some(v) if v.value() == "1" => 1u32,
                Some(v) if v.value() == "2" => 2u32,
                Some(v) if v.value() == "3" => 3u32,
                Some(v) if v.value() == "4" => 4u32,
                Some(v) if v.value() == "5" => 5u32,
                Some(v) if v.value() == "6" => 6u32,
                Some(v) if v.value() == "7" => 7u32,
                Some(v) if v.value() == "8" => 8u32,
                Some(v) if v.value() == "9" => 9u32,
                Some(v) if v.value() == "10" => 10u32,
                Some(v) if v.value() == "11" => 11u32,
                Some(v) => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: v.value().to_owned(),
                    });
                }
            }
        };
        control.check()?;
        let tx = db.begin_write()?;
        {
            tx.open_table(SEEN)?;
            let mut meta = tx.open_table(META)?;
            if from == 1 {
                // The v2 steps, unchanged: initialize revision/scan metadata
                // and derive the bound workspace identity.
                meta.insert("source_revision", "0")?;
                meta.insert("scan_id", "0")?;
                meta.insert("scan_status", "never")?;
                let bound_root = meta.get("workspace")?.map(|v| v.value().to_owned());
                if let Some(root) = bound_root {
                    let id = crate::digest(root.as_bytes());
                    meta.insert("workspace_id", id.as_str())?;
                }
            }
            if from < 3 {
                // The v3 steps: the memory table (empty; records arrive only
                // by explicit puts) and its revision counter, which starts
                // at 0 and is never reset, even when the table is empty
                // again. A v3 store keeps both untouched.
                tx.open_table(MEMORY)?;
                meta.insert("memory_revision", "0")?;
                // Typed pending keys: every pre-v3 key is a raw source path
                // (v3 writers have always typed theirs), so prefix them all.
                // A path may itself contain `:` — `source:memory:x` stays
                // distinct from the memory key `memory:x`.
                let mut pending = tx.open_table(PENDING)?;
                let rows: Vec<(String, String)> = pending
                    .iter()?
                    .map(|row| {
                        let (k, v) = row?;
                        Ok((k.value().to_owned(), v.value().to_owned()))
                    })
                    .collect::<Result<Vec<_>, redb::StorageError>>()
                    .map_err(FoundryError::from)?;
                // Remove EVERY original key before inserting any typed key:
                // interleaving would overwrite a raw `source:a` row while
                // migrating `a`, losing that path's pending work.
                for (key, _) in &rows {
                    pending.remove(key.as_str())?;
                }
                for (key, value) in rows {
                    pending.insert(source_pending_key(&key).as_str(), value.as_str())?;
                }
            }
            // The v4 steps (005): the empty compiler-fact tables. Facts
            // arrive only by an explicit `import-scip`.
            if from < 4 {
                crate::graph::init_compiler(&tx)?;
            }
            // The v5 steps (009): the empty semantic tables. Rows arrive
            // only by an explicit `semantic prepare`.
            if from < 5 {
                crate::neural::cache::init_tables(&tx)?;
            }
            // The v6 steps (013): the empty learning tables. Rows arrive
            // only by explicit operator `feedback v4` input and `learning
            // prepare`.
            if from < 6 {
                crate::learning::init_tables(&tx)?;
            }
            if from < 7 {
                tx.open_table(crate::task_context::TASKS)?;
                meta.insert("task_revision", "0")?;
            }
            if from < 8 {
                tx.open_table(BRIEFS)?;
                meta.insert("brief_revision", "0")?;
            }
            crate::neural::work::init(&tx)?;
            crate::neural::mapping_store::init(&tx)?;
            crate::knowledge::cache::init(&tx)?;
            crate::boundary::init(&tx)?;
            // Publish the schema last inside the same transaction.
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
        }
        // The upgrade transaction is live and fully written but uncommitted:
        // an exit here must leave the store wholly v1/v2/v3/v4.
        fault!(UPGRADE_BEFORE_COMMIT, None, Some(control), "")?;
        control.check()?;
        tx.commit()?;
        fault!(UPGRADE_AFTER_COMMIT, None, Some(control), "")?;
        Ok(())
    }

    pub fn status(&self) -> FResult<StoreStatus> {
        let (revision, sources, pending, scan_state) = {
            let tx = self.db.begin_read()?;
            let meta = tx.open_table(META)?;
            let revision = read_counter(&meta, "source_revision")?;
            let sources = tx.open_table(SOURCES)?.len()?;
            let pending = tx.open_table(PENDING)?.len()?;
            let scan_state = read_scan_state(&meta)?;
            (revision, sources, pending, scan_state)
        };
        let (index_state, index_reason) = if self.schema != SCHEMA_VERSION {
            ("unsupported".to_owned(), None)
        } else if let Some(reason) = &self.repair_reason {
            ("repair_required".to_owned(), Some(reason.clone()))
        } else if self.search.is_some() {
            if pending == 0 {
                ("ready".to_owned(), None)
            } else {
                ("lagging".to_owned(), None)
            }
        } else {
            (
                "repair_required".to_owned(),
                Some("derived index unavailable".into()),
            )
        };
        // Sources left as plain blocks after a parse panic stay named until
        // parsed again. The derivation reads the serving index only; a read
        // failure leaves the field unknown, never an invented zero, and
        // status itself stays available.
        let parse_failures = match (&self.search, &self.repair_reason) {
            (Some(handles), None) if self.schema == SCHEMA_VERSION => {
                unparsed_sources(&handles.reader.searcher(), &handles.fields).ok()
            }
            _ => None,
        };
        Ok(StoreStatus {
            schema: self.schema,
            workspace_id: self.workspace_id.clone(),
            source_revision: revision,
            source_count: sources,
            pending_count: pending,
            index_state,
            index_reason,
            scan_state,
            parse_failures: parse_failures.as_ref().map(|named| named.count),
            parse_failure_samples: parse_failures
                .map(|named| named.samples)
                .unwrap_or_default(),
        })
    }

    fn persist_marker(&self, marker: &RebuildMarker) -> FResult<()> {
        let tx = self.db.begin_write()?;
        tx.open_table(META)?.insert(
            "search_rebuild_required",
            serde_json::to_string(marker)?.as_str(),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Explicit derived-index repair under exclusive ownership: durable
    /// marker, one retained quarantine, paged enqueue, drained replacement.
    pub fn repair_index(store_dir: &Path, control: &crate::Control) -> FResult<RepairReport> {
        control.check()?;
        let mut engine = Self::open_with(store_dir, false, true)?;
        if engine.schema != SCHEMA_VERSION {
            return Err(FoundryError::UpgradeRequired {
                found: engine.schema.to_string(),
            });
        }
        let existing = Self::read_marker(&engine.db)?;
        let search_dir = store_dir.join("search");
        let mut marker = existing.unwrap_or_else(|| {
            let nonce = format!(
                "{}:{}:{}",
                std::process::id(),
                engine.directory.display(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            RebuildMarker {
                owner_id: crate::digest(nonce.as_bytes()),
                quarantine: None,
                original_handled: false,
            }
        });
        // Decide the quarantine action BEFORE persisting a marker, so a refused
        // path (symlink, conflicting quarantine) leaves a healthy index usable.
        enum Quarantine {
            Done,
            Move { basename: String },
            AdoptCompletedMove,
            NothingToRetain,
        }
        let quarantine = if marker.original_handled {
            Quarantine::Done
        } else {
            if search_dir.is_symlink() {
                return Err(FoundryError::RepairPathConflict(
                    "search path is a symlink".into(),
                ));
            }
            let basename = marker
                .quarantine
                .clone()
                .unwrap_or_else(|| format!("search.quarantine-{}", &marker.owner_id[..12]));
            let search_exists = search_dir.exists();
            let target_exists = store_dir.join(&basename).symlink_metadata().is_ok();
            match (search_exists, target_exists) {
                (true, false) => Quarantine::Move { basename },
                // The rename finished before an earlier crash: adopt it.
                (false, true) if marker.quarantine.is_some() => Quarantine::AdoptCompletedMove,
                // No original derived directory existed: nothing to retain.
                (false, false) => Quarantine::NothingToRetain,
                _ => {
                    return Err(FoundryError::RepairPathConflict(format!(
                        "quarantine {basename} conflicts with the current search path"
                    )));
                }
            }
        };
        // Persist the marker before touching any derived file.
        engine.persist_marker(&marker)?;
        fault!(REPAIR_AFTER_MARKER, Some(&engine), Some(control), "")?;
        control.check()?;
        match quarantine {
            Quarantine::Done => {}
            Quarantine::Move { basename } => {
                // Record the move as intent first, so a crash on either side
                // of the rename is recognizable on retry.
                marker.quarantine = Some(basename.clone());
                engine.persist_marker(&marker)?;
                fault!(
                    REPAIR_BEFORE_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                std::fs::rename(&search_dir, store_dir.join(&basename)).map_err(|e| {
                    FoundryError::RepairPathConflict(format!("quarantine move failed: {e}"))
                })?;
                fault!(
                    REPAIR_AFTER_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::AdoptCompletedMove => {
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::NothingToRetain => {
                marker.quarantine = None;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
        }
        let quarantined_to = marker.quarantine.as_ref().map(|b| store_dir.join(b));
        control.check()?;
        // The replacement directory is reset only when it positively carries
        // this repair's identity: a regular `rebuild_id` file whose nonempty
        // content equals the marker's owner id. A completely empty directory
        // holds nothing to lose (a crash between creating it and writing the
        // identity) and is adopted. Everything else, including a missing,
        // empty, malformed or symlinked identity beside other files, is a
        // conflict: nothing is moved or deleted.
        let id_file = search_dir.join("rebuild_id");
        if search_dir.is_symlink() {
            return Err(FoundryError::RepairPathConflict(
                "replacement search path is a symlink".into(),
            ));
        }
        if search_dir.exists() {
            let matched = std::fs::symlink_metadata(&id_file)
                .ok()
                .filter(|meta| meta.file_type().is_file())
                .and_then(|_| std::fs::read_to_string(&id_file).ok())
                .is_some_and(|content| {
                    let content = content.trim();
                    !content.is_empty() && content == marker.owner_id
                });
            let empty = std::fs::read_dir(&search_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if matched {
                for entry in std::fs::read_dir(&search_dir)? {
                    let entry = entry?;
                    if entry.file_name() == "rebuild_id" {
                        continue;
                    }
                    // file_type() does not follow symlinks: a link is removed
                    // as a link, never traversed.
                    if entry.file_type()?.is_dir() {
                        std::fs::remove_dir_all(entry.path())?;
                    } else {
                        std::fs::remove_file(entry.path())?;
                    }
                }
            } else if empty {
                std::fs::write(&id_file, &marker.owner_id)?;
            } else {
                return Err(FoundryError::RepairPathConflict(
                    "replacement directory does not carry this repair's identity".into(),
                ));
            }
        } else {
            std::fs::create_dir_all(&search_dir)?;
            std::fs::write(&id_file, &marker.owner_id)?;
        }
        // Enqueue every source version in pages.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page: Vec<(String, String)> = {
                let tx = engine.db.begin_read()?;
                let sources = tx.open_table(SOURCES)?;
                let rows = match &after {
                    None => sources.range::<&str>(..)?,
                    Some(key) => {
                        sources.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| {
                        let (k, v) = row?;
                        Ok((k.value().to_owned(), v.value().to_owned()))
                    })
                    .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (path, hash) in &page {
                    pending.insert(source_pending_key(path).as_str(), hash.as_str())?;
                }
            }
            tx.commit()?;
            fault!(REPAIR_AFTER_ENQUEUE_PAGE, Some(&engine), Some(control), "")?;
            after = page.last().map(|(k, _)| k.clone());
        }
        // Enqueue every live memory record (008): the replacement index is
        // derived, so repair rebuilds memory documents from the live table.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page: Vec<(String, String)> = {
                let tx = engine.db.begin_read()?;
                let memory = tx.open_table(MEMORY)?;
                let rows = match &after {
                    None => memory.range::<&str>(..)?,
                    Some(key) => {
                        memory.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| {
                        let (k, v) = row?;
                        // An undecodable row still queues (so a stale document
                        // is dropped) under a marker value; it gets no document.
                        let revision = serde_json::from_str::<MemoryRecord>(v.value())
                            .map_or_else(|_| "corrupt".to_owned(), |r| r.revision.to_string());
                        Ok((k.value().to_owned(), revision))
                    })
                    .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (id, revision) in &page {
                    pending.insert(memory_pending_key(id).as_str(), revision.as_str())?;
                }
            }
            tx.commit()?;
            after = page.last().map(|(k, _)| k.clone());
        }
        control.check()?;
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = {
                let tx = engine.db.begin_read()?;
                let table = tx.open_table(BRIEFS)?;
                table
                    .range::<&str>((
                        after.as_deref().map_or(Bound::Unbounded, Bound::Excluded),
                        Bound::Unbounded,
                    ))?
                    .take(PAGE)
                    .map(|row| {
                        let (key, raw) = row?;
                        let revision = crate::brief::decode_row(key.value(), raw.value())
                            .map_or_else(|_| "corrupt".into(), |r| r.revision.to_string());
                        Ok((key.value().to_owned(), revision))
                    })
                    .collect::<Result<Vec<_>, redb::StorageError>>()?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (id, revision) in &page {
                    pending.insert(crate::brief::pending_key(id).as_str(), revision.as_str())?;
                }
            }
            tx.commit()?;
            after = page.last().map(|(id, _)| id.clone());
        }
        // Construct the replacement index and drain.
        fault!(
            REPAIR_BEFORE_REPLACEMENT_INDEX,
            Some(&engine),
            Some(control),
            ""
        )?;
        let index = Index::create_in_dir(&search_dir, search_schema())?;
        engine.search = Some(search_handles(index)?);
        fault!(
            REPAIR_AFTER_REPLACEMENT_INDEX,
            Some(&engine),
            Some(control),
            ""
        )?;
        engine.repair_reason = None;
        let mut drained = (0usize, 0usize);
        loop {
            control.check()?;
            match engine.refresh_index(control)? {
                (0, 0) if engine.pending()? == 0 => break,
                (sources, memories) => {
                    drained.0 += sources;
                    drained.1 += memories;
                }
            }
        }
        // Clear the marker only after successful index commit/reload and an
        // empty pending table. Cancellation here leaves the marker set.
        if engine.pending()? != 0 {
            return Err(FoundryError::RepairRequired(
                "pending work remains after drain; marker retained".into(),
            ));
        }
        fault!(REPAIR_BEFORE_MARKER_CLEAR, Some(&engine), Some(control), "")?;
        control.check()?;
        {
            // The current index format is published in the same transaction
            // that clears the marker.
            let tx = engine.db.begin_write()?;
            {
                let mut meta = tx.open_table(META)?;
                meta.remove("search_rebuild_required")?;
                meta.insert("search_schema", SEARCH_SCHEMA)?;
            }
            tx.commit()?;
        }
        fault!(
            REPAIR_AFTER_SCHEMA_PUBLICATION,
            Some(&engine),
            Some(control),
            ""
        )?;
        // 009: a lexical repair also replays the semantic generation from
        // the retained f32 cache — derived state both, zero document calls.
        // A semantic failure is named in the report, never a rollback of the
        // lexical repair.
        engine.semantic_reconcile_cache()?;
        #[cfg(feature = "semantic")]
        let semantic_index = Some(engine.semantic_rebuild_index(control).unwrap_or_else(|e| {
            crate::neural::index::SemanticIndexReport {
                rebuilt: false,
                entries: 0,
                reason: Some(e.to_string()),
            }
        }));
        #[cfg(not(feature = "semantic"))]
        let semantic_index: Option<crate::neural::index::SemanticIndexReport> = None;
        Ok(RepairReport {
            repaired: true,
            quarantined_to,
            drained_sources: drained.0,
            drained_memory: drained.1,
            reason: None,
            semantic_index,
        })
    }
}
