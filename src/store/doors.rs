//! Doors (context-v2 § Doors): approximate-door candidates and the verified
//! file views they read, and the `Engine` door reads that context assembly
//! calls. The door result types stay in the parent module.
use super::*;

/// One approximate-door candidate (context-v2 § Doors): a delivery unit
/// whose `ident` holds the definition's name, with its door-order keys.
struct DoorCandidate {
    /// Its file's import keys hold the name or the definition's module.
    importing: bool,
    role: u64,
    path: String,
    hash: String,
    unit_start: u64,
    unit_end: u64,
    /// `<kind> <qualified name>`, or the kind alone.
    label: String,
}

/// One verified source an approximate-doors pass reads, with its line starts.
struct DoorFile {
    hash: String,
    body: String,
    line_starts: Vec<usize>,
}

impl DoorFile {
    fn new(hash: String, body: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(body.match_indices('\n').map(|(at, _)| at + 1));
        Self {
            hash,
            body,
            line_starts,
        }
    }

    /// One-based line of the byte `offset`.
    fn line_of(&self, offset: usize) -> u64 {
        self.line_starts.partition_point(|&start| start <= offset) as u64
    }

    /// The text of one-based `line`, without its LF or CRLF terminator.
    fn line_text(&self, line: u64) -> &str {
        let index = (line as usize).saturating_sub(1);
        let start = self
            .line_starts
            .get(index)
            .copied()
            .unwrap_or(self.body.len());
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.body.len());
        let text = &self.body[start..end];
        let text = text.strip_suffix('\n').unwrap_or(text);
        text.strip_suffix('\r').unwrap_or(text)
    }
}

/// `path`'s verified bytes, loaded into `files` once in the caller's read
/// transaction; `None` when the source is absent or no longer carries
/// `hash`.
fn door_file<'f>(
    files: &'f mut std::collections::HashMap<String, Option<DoorFile>>,
    sources: &redb::ReadOnlyTable<&'static str, &'static str>,
    stored: &redb::ReadOnlyTable<&'static str, &'static str>,
    path: &str,
    hash: &str,
) -> FResult<Option<&'f DoorFile>> {
    if !files.contains_key(path) {
        let file = match sources.get(path)? {
            Some(raw) => {
                let meta = decode::<SourceMeta>(raw.value(), "source")?;
                let body = reconstruct_verified(stored, path, &meta)?.body;
                Some(DoorFile::new(meta.hash, body))
            }
            None => None,
        };
        files.insert(path.to_owned(), file);
    }
    Ok(files[path].as_ref().filter(|file| file.hash == hash))
}

/// The last segment of a definition's module (context-v2 § Doors): its file
/// stem, or its directory for an `index`, `mod`, `lib` or `__init__` stem and
/// for every Go file (its package).
fn module_key(path: &str) -> Option<&str> {
    let (directory, file) = match path.rsplit_once('/') {
        Some((directory, file)) => (Some(directory), file),
        None => (None, path),
    };
    let stem = Path::new(file).file_stem()?.to_str()?;
    if path.ends_with(".go") || matches!(stem, "index" | "mod" | "lib" | "__init__") {
        directory.map(|directory| directory.rsplit('/').next().unwrap_or(directory))
    } else {
        Some(stem)
    }
}

/// The byte offset of the first occurrence of `name` in `text` exactly as
/// written and as a whole identifier: no identifier character (alphanumeric,
/// `_` or `$`) touches it on either side.
fn find_word(text: &str, name: &str) -> Option<usize> {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).find_map(|(at, _)| {
        let before = text[..at].chars().next_back().is_some_and(identifier);
        let after = text[at + name.len()..]
            .chars()
            .next()
            .is_some_and(identifier);
        (!before && !after).then_some(at)
    })
}

impl Engine {
    /// The doors of a context that requested them (context-v2 § Doors),
    /// built in its final read `tx` for `target`, decided from the first
    /// anchor's window before this read. A target this read's validation of
    /// the anchor windows dropped as stale gets no doors, and no other
    /// definition takes its place: a resolved `D` gives `none`.
    ///
    /// Each surviving target reads one references window, exactly the
    /// resolved anchor's (at most [`TIE_GROUP`] per request), with a
    /// cancellation check before every window after the first. Exact doors
    /// when a current compiler scope of a target's path holds definition
    /// occurrences at its stored name range. A resolved `D` without them
    /// gets approximate doors; a tie group's entry without them gets no
    /// group, and only when no entry has exact doors does the name get
    /// approximate doors once, attributed to no entry. A malformed compiler
    /// row is component-local: it gives approximate doors, never a failed
    /// context.
    pub(super) fn doors_in(
        &self,
        tx: &redb::ReadTransaction,
        target: &DoorTarget,
        anchors: &[AnchorWindow],
        counters: &mut CandidateCounters,
        control: &crate::Control,
    ) -> FResult<Doors> {
        let (targets, cap) = match target {
            DoorTarget::None => return Ok(Doors::unbuilt(DoorState::None)),
            DoorTarget::Ambiguous => return Ok(Doors::unbuilt(DoorState::Ambiguous)),
            DoorTarget::Resolved(handle) => (std::slice::from_ref(handle), DOOR_FILES),
            DoorTarget::Tied(handles) => (handles.as_slice(), GROUP_FILES),
        };
        // Validation only removes entries: a target survived exactly when it
        // is still in the first anchor's window.
        let surviving = |handle: &SourceHandle| {
            anchors
                .first()?
                .entries
                .iter()
                .find(|entry| entry.handle.as_ref() == Some(handle))?
                .resolver
        };
        let revision = self.freshness_in(tx)?.source_revision;
        let bound = self.workspace_id.clone().unwrap_or_default();
        let mut reads = graph::DoorReads::new(tx, revision, &bound);
        let mut groups: Vec<DoorGroup> = Vec::new();
        // The first surviving target names the approximate doors.
        let mut named: Option<(&SourceHandle, Resolver)> = None;
        for handle in targets {
            let Some(resolver) = surviving(handle) else {
                continue;
            };
            match named {
                // Every window after the first checks the request's
                // cancellation.
                Some(_) => control.check()?,
                None => named = Some((handle, resolver)),
            }
            match reads.exact_doors(&handle.path, &handle.sha256, resolver.name, cap) {
                Ok(graph::ExactResolution::Doors(exact)) => {
                    counters.stale += exact.stale as u64;
                    counters.candidates_full |= exact.full;
                    groups.push(DoorGroup {
                        target: Some(handle.clone()),
                        lines: exact.lines,
                        more_files: exact.more_files,
                    });
                }
                // A name scan that ran out of its allowance filled the
                // window, whatever the approximate window does.
                Ok(graph::ExactResolution::Approximate { exhausted }) => {
                    counters.candidates_full |= exhausted;
                }
                Err(FoundryError::GraphInvalid(_)) => {}
                Err(other) => return Err(other),
            }
        }
        let resolved = matches!(target, DoorTarget::Resolved(_));
        if !groups.is_empty() {
            let state = if resolved {
                DoorState::Exact
            } else {
                DoorState::Each
            };
            return Ok(Doors { state, groups });
        }
        let Some(group) = named
            .map(|(handle, resolver)| {
                self.approx_doors(tx, handle, resolver.name, targets, counters, control)
            })
            .transpose()?
            .flatten()
        else {
            return Ok(Doors::unbuilt(DoorState::None));
        };
        Ok(Doors {
            state: DoorState::Approx,
            groups: vec![DoorGroup {
                target: resolved.then(|| targets[0].clone()),
                ..group
            }],
        })
    }

    /// Approximate doors (context-v2 § Doors) for the name exactly as
    /// written at `name` in `named`'s verified bytes, without a trailing
    /// `?`, `!` or `'`: the units of `own` are excluded and their module keys
    /// are import keys. One group, onto no target; `None` when `named`'s
    /// source is gone from this read.
    fn approx_doors(
        &self,
        tx: &redb::ReadTransaction,
        named: &SourceHandle,
        name: (u64, u64),
        own: &[SourceHandle],
        counters: &mut CandidateCounters,
        control: &crate::Control,
    ) -> FResult<Option<DoorGroup>> {
        let bound = self.workspace_id.clone().unwrap_or_default();
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut files: std::collections::HashMap<String, Option<DoorFile>> =
            std::collections::HashMap::new();
        let Some(file) = door_file(&mut files, &sources, &stored, &named.path, &named.sha256)?
        else {
            return Ok(None);
        };
        let (start, end) = (name.0 as usize, name.1 as usize);
        let name = file
            .body
            .get(start..end)
            .ok_or_else(|| {
                FoundryError::CorruptStore(format!(
                    "a stored name range lies outside {}",
                    named.path
                ))
            })?
            .trim_end_matches(['?', '!', '\''])
            .to_owned();
        let mut doors = DoorGroup {
            target: None,
            lines: Vec::new(),
            more_files: 0,
        };
        // Identifiers of one character have no doors.
        if name.chars().count() < 2 {
            return Ok(Some(doors));
        }
        let (candidates, full) = self.door_candidates(&name, own, control)?;
        counters.candidates_full |= full;
        // A candidate becomes a door where a line of it holds the name
        // exactly as written; one site per unit, grouped by file in door
        // order (a file's first candidate is its first site).
        let mut groups: Vec<(DoorLine, usize)> = Vec::new();
        let mut group_of: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for candidate in &candidates {
            control.check()?;
            let Some(file) = door_file(
                &mut files,
                &sources,
                &stored,
                &candidate.path,
                &candidate.hash,
            )?
            else {
                counters.stale += 1;
                continue;
            };
            let (start, end) = (candidate.unit_start as usize, candidate.unit_end as usize);
            let Some(text) = file.body.get(start..end) else {
                return Err(FoundryError::CorruptStore(format!(
                    "search document unit outside {}",
                    candidate.path
                )));
            };
            let Some(at) = find_word(text, &name) else {
                continue;
            };
            match group_of.get(&candidate.path) {
                Some(&index) => groups[index].1 += 1,
                None => {
                    group_of.insert(candidate.path.clone(), groups.len());
                    let line = file.line_of(start + at);
                    groups.push((
                        DoorLine {
                            unit: SourceHandle {
                                workspace_id: bound.clone(),
                                path: candidate.path.clone(),
                                sha256: candidate.hash.clone(),
                                start: candidate.unit_start,
                                end: candidate.unit_end,
                            },
                            line,
                            label: candidate.label.clone(),
                            text: file.line_text(line).to_owned(),
                            more: 0,
                        },
                        0,
                    ));
                }
            }
        }
        doors.more_files = groups.len().saturating_sub(DOOR_FILES);
        doors.lines = groups
            .into_iter()
            .take(DOOR_FILES)
            .map(|(line, more)| DoorLine { more, ..line })
            .collect();
        Ok(Some(doors))
    }

    /// The approximate-door candidates of the definitions `own`, all named
    /// `name` (context-v2 § Doors): the delivery units, other than theirs,
    /// whose `ident` holds the name, in door order - importing files first (a
    /// file whose index-time import keys hold the name or one of their
    /// module keys), then role, path and start - cut to the first
    /// [`DOOR_WINDOW`]; and whether more existed. The request's cancellation
    /// is checked as each search document is read. Nothing is resolved, read
    /// from configuration or executed: the keys were computed at indexing.
    fn door_candidates(
        &self,
        name: &str,
        own: &[SourceHandle],
        control: &crate::Control,
    ) -> FResult<(Vec<DoorCandidate>, bool)> {
        let handles = self.require_search()?;
        let fields = &handles.fields;
        let searcher = handles.reader.searcher();
        let term = |field: Field, text: &str| -> Box<dyn Query> {
            Box::new(TermQuery::new(
                Term::from_field_text(field, text),
                IndexRecordOption::Basic,
            ))
        };
        let mut modules: Vec<&str> = vec![name];
        for module in own.iter().filter_map(|handle| module_key(&handle.path)) {
            if !modules.contains(&module) {
                modules.push(module);
            }
        }
        let keys = modules
            .into_iter()
            .map(|key| term(fields.imports, key))
            .collect();
        let mut importing: std::collections::HashSet<String> = std::collections::HashSet::new();
        for address in searcher.search(
            &BooleanQuery::union(keys),
            &tantivy::collector::DocSetCollector,
        )? {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            if let Some(path) = doc.get_first(fields.path).and_then(|v| v.as_str()) {
                importing.insert(path.to_owned());
            }
        }
        let mentions = BooleanQuery::new(vec![
            (Occur::Must, term(fields.ident, &name.to_lowercase())),
            (Occur::MustNot, term(fields.kind, "memory")),
            (Occur::MustNot, term(fields.kind, "brief")),
        ]);
        let mut units: std::collections::BTreeMap<(String, u64, u64), DoorCandidate> =
            std::collections::BTreeMap::new();
        for address in searcher.search(&mentions, &tantivy::collector::DocSetCollector)? {
            control.check()?;
            let doc: TantivyDocument = searcher.doc(address)?;
            let invalid = || FoundryError::CorruptStore("invalid search document".into());
            let text = |field| doc.get_first(field).and_then(|v| v.as_str());
            let number = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(invalid)
            };
            let path = text(fields.path).ok_or_else(invalid)?;
            let (unit_start, unit_end) = (number(fields.unit_start)?, number(fields.unit_end)?);
            let own_unit = own.iter().any(|handle| {
                handle.path == path && handle.start == unit_start && handle.end == unit_end
            });
            if own_unit {
                continue;
            }
            let key = (path.to_owned(), unit_start, unit_end);
            if units.contains_key(&key) {
                continue;
            }
            let kind = match text(fields.kind).ok_or_else(invalid)? {
                UNPARSED_KIND => crate::syntax::UnitKind::Block.as_str(),
                kind => kind,
            };
            let label = match text(fields.qname) {
                Some(qname) => format!("{kind} {qname}"),
                None => kind.to_owned(),
            };
            units.insert(
                key,
                DoorCandidate {
                    importing: importing.contains(path),
                    role: number(fields.role)?,
                    path: path.to_owned(),
                    hash: text(fields.hash).ok_or_else(invalid)?.to_owned(),
                    unit_start,
                    unit_end,
                    label,
                },
            );
        }
        let mut candidates: Vec<DoorCandidate> = units.into_values().collect();
        candidates.sort_by(|a, b| {
            (!a.importing, a.role, &a.path, a.unit_start).cmp(&(
                !b.importing,
                b.role,
                &b.path,
                b.unit_start,
            ))
        });
        let full = candidates.len() > DOOR_WINDOW;
        candidates.truncate(DOOR_WINDOW);
        Ok((candidates, full))
    }
}
