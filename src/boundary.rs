//! 005 T005: explicitly scoped, syntactic HTTP relationships. No workspace code
//! runs. Scope replacement and key postings share one redb transaction; joins
//! are resolved at read time against current source/configuration identities.
use crate::store::{CHUNKS, SOURCES, reconstruct_verified, validate_path};
use crate::{Control, Engine, FResult, FoundryError, SourceHandle, SourceMeta};
use redb::{MultimapTableDefinition, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::Node;

const SCOPES: TableDefinition<&str, &str> = TableDefinition::new("boundary_scopes");
const KEYS: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("boundary_keys");
pub const RECIPE: &str = "python-http-v1";
const MAX_FILES: usize = 64;
const MAX_FACTS: usize = 128;
const MAX_RESULTS: usize = 64;

fn invalid(s: impl Into<String>) -> FoundryError {
    FoundryError::InvalidArgument(s.into())
}
fn decode<T: serde::de::DeserializeOwned>(s: &str) -> FResult<T> {
    serde_json::from_str(s).map_err(|e| FoundryError::GraphInvalid(format!("boundary row: {e}")))
}
pub(crate) fn init(tx: &redb::WriteTransaction) -> FResult<()> {
    tx.open_table(SCOPES)?;
    tx.open_multimap_table(KEYS)?;
    Ok(())
}
pub(crate) fn check(tx: &redb::ReadTransaction) -> FResult<()> {
    tx.open_table(SCOPES)
        .map_err(|e| FoundryError::CorruptStore(format!("boundary scopes: {e}")))?;
    tx.open_multimap_table(KEYS)
        .map_err(|e| FoundryError::CorruptStore(format!("boundary keys: {e}")))?;
    Ok(())
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub v: u32,
    pub services: Vec<Service>,
    pub consumers: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub id: String,
    pub providers: Vec<String>,
    pub origins: Vec<String>,
}
impl Config {
    fn paths(&self) -> BTreeSet<String> {
        self.services
            .iter()
            .flat_map(|s| s.providers.iter())
            .chain(self.consumers.iter())
            .cloned()
            .collect()
    }
    fn validate(&self) -> FResult<()> {
        if self.v != 1
            || self.services.is_empty()
            || self.services.len() > 16
            || self.paths().len() > MAX_FILES
            || self.consumers.len() > MAX_FILES
        {
            return Err(invalid(
                "boundary config requires v1, 1..16 services and at most 64 distinct files",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut origins = BTreeSet::new();
        let mut providers = BTreeSet::new();
        for s in &self.services {
            if s.id.is_empty()
                || s.id.len() > 64
                || !s
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !ids.insert(&s.id)
                || s.providers.is_empty()
                || s.providers.len() > MAX_FILES
                || s.origins.is_empty()
                || s.origins.len() > 16
            {
                return Err(invalid("invalid or duplicate boundary service"));
            }
            for p in &s.providers {
                if !providers.insert(p) {
                    return Err(invalid("provider path belongs to multiple service entries"));
                }
            }
            for o in &s.origins {
                if origin(o).as_deref() != Some(o) || !origins.insert(o) {
                    return Err(invalid(
                        "origins must be unique canonical HTTP origins without paths or credentials",
                    ));
                }
            }
        }
        for p in self.paths() {
            validate_path(&p)?;
            if !p.ends_with(".py") {
                return Err(invalid("python-http-v1 accepts only .py sources"));
            }
        }
        Ok(())
    }
}
fn origin(s: &str) -> Option<String> {
    let u = reqwest::Url::parse(s).ok()?;
    if !["http", "https"].contains(&u.scheme())
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || u.path() != "/"
    {
        return None;
    }
    Some(u.origin().ascii_serialization())
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub service: String,
    pub method: String,
    pub path: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Span {
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
}
impl Span {
    pub fn handle(&self, workspace_id: &str) -> SourceHandle {
        SourceHandle {
            workspace_id: workspace_id.into(),
            path: self.path.clone(),
            sha256: self.sha256.clone(),
            start: self.start,
            end: self.end,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fact {
    pub role: String,
    pub key: Option<Key>,
    pub evidence: Span,
    pub context: Span,
    pub unsupported: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Scope {
    recipe: String,
    config_hash: String,
    source_hash: String,
    complete: bool,
    facts: Vec<Fact>,
}
#[derive(Debug, Serialize)]
pub struct Publication {
    pub recipe: &'static str,
    pub parsed: usize,
    pub reused: usize,
    pub removed: usize,
    pub facts: usize,
    pub incomplete: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Relation {
    pub kind: &'static str,
    pub resolution: &'static str,
    pub consumer: Fact,
    pub providers: Vec<Fact>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Relations {
    pub recipe: &'static str,
    pub evidence_class: &'static str,
    pub workspace_id: String,
    pub source_revision: u64,
    pub config: Span,
    pub scope: &'static str,
    pub relations: Vec<Relation>,
    pub incomplete_paths: Vec<String>,
    pub truncated: bool,
    pub examined: usize,
}
fn scope_key(config: &str, path: &str) -> String {
    format!("{}:{path}", crate::digest(config.as_bytes()))
}
fn posting(config: &str, key: &Key) -> String {
    format!(
        "{}:{}",
        crate::digest(config.as_bytes()),
        crate::digest(
            serde_json::to_string(key)
                .expect("key serializes")
                .as_bytes()
        )
    )
}
pub(crate) fn source_in(
    tx: &redb::ReadTransaction,
    path: &str,
) -> FResult<Option<(SourceMeta, String)>> {
    let sources = tx.open_table(SOURCES)?;
    let Some(raw) = sources.get(path)? else {
        return Ok(None);
    };
    let meta: SourceMeta = crate::store::decode(raw.value(), "source")?;
    let body = reconstruct_verified(&tx.open_table(CHUNKS)?, path, &meta)?.body;
    Ok(Some((meta, body)))
}
fn config_in(tx: &redb::ReadTransaction, path: &str) -> FResult<(Config, Span)> {
    validate_path(path)?;
    let (meta, body) = source_in(tx, path)?.ok_or(FoundryError::NotFound)?;
    if body.len() > 64 * 1024 {
        return Err(invalid("boundary config exceeds 64 KiB"));
    }
    let config: Config =
        serde_json::from_str(&body).map_err(|e| invalid(format!("boundary config: {e}")))?;
    config.validate()?;
    Ok((
        config,
        Span {
            path: path.into(),
            sha256: meta.hash,
            start: 0,
            end: body.len() as u64,
        },
    ))
}
impl Engine {
    /// Empty paths refreshes the bounded configuration. Explicit paths replace
    /// only those files; hashes still suppress every stale endpoint at read time.
    pub fn index_boundaries(
        &self,
        config_path: &str,
        paths: &[String],
        control: &Control,
    ) -> FResult<Publication> {
        control.check()?;
        if paths.len() > MAX_FILES {
            return Err(invalid("boundary refresh accepts at most 64 paths"));
        }
        let read = self.db.begin_read()?;
        let (config, config_span) = config_in(&read, config_path)?;
        let admitted = config.paths();
        let selected: BTreeSet<String> = if paths.is_empty() {
            admitted.clone()
        } else {
            paths.iter().cloned().collect()
        };
        if !selected.is_subset(&admitted) {
            return Err(invalid(
                "boundary refresh paths must belong to the indexed config",
            ));
        }
        let scopes = read.open_table(SCOPES)?;
        let sources = read.open_table(SOURCES)?;
        let mut report = Publication {
            recipe: RECIPE,
            parsed: 0,
            reused: 0,
            removed: 0,
            facts: 0,
            incomplete: 0,
        };
        let mut changes = Vec::new();
        for path in selected {
            control.check()?;
            let previous: Option<Scope> = scopes
                .get(scope_key(config_path, &path).as_str())?
                .map(|r| decode(r.value()))
                .transpose()?;
            let meta: Option<SourceMeta> = sources
                .get(path.as_str())?
                .map(|r| crate::store::decode(r.value(), "source"))
                .transpose()?;
            if let (Some(old), Some(meta)) = (&previous, &meta)
                && old.recipe == RECIPE
                && old.config_hash == config_span.sha256
                && old.source_hash == meta.hash
            {
                report.reused += 1;
                continue;
            }
            let next = if let Some(meta) = &meta {
                let body = reconstruct_verified(&read.open_table(CHUNKS)?, &path, meta)?.body;
                let (facts, complete) = extract(&config, &path, &meta.hash, &body, control)?;
                report.parsed += 1;
                report.facts += facts.len();
                report.incomplete += usize::from(!complete);
                Some(Scope {
                    recipe: RECIPE.into(),
                    config_hash: config_span.sha256.clone(),
                    source_hash: meta.hash.clone(),
                    complete,
                    facts,
                })
            } else {
                report.removed += usize::from(previous.is_some());
                None
            };
            changes.push((path, meta.map(|m| m.hash), previous, next));
        }
        // Configuration edits retire removed paths in the same publication.
        // The prefix is one admitted configuration, never the corpus table.
        let prefix = crate::digest(config_path.as_bytes());
        let lower = format!("{prefix}:");
        let upper = format!("{prefix};");
        for (examined, row) in scopes.range(lower.as_str()..upper.as_str())?.enumerate() {
            control.check()?;
            if examined >= MAX_FILES {
                return Err(FoundryError::GraphInvalid(
                    "boundary configuration has too many stored scopes".into(),
                ));
            }
            let (key, raw) = row?;
            let path = &key.value()[lower.len()..];
            if !admitted.contains(path) {
                let old: Scope = decode(raw.value())?;
                let expected = sources
                    .get(path)?
                    .map(|r| {
                        crate::store::decode::<SourceMeta>(r.value(), "source").map(|m| m.hash)
                    })
                    .transpose()?;
                changes.push((path.to_owned(), expected, Some(old), None));
                report.removed += 1;
            }
        }
        drop(scopes);
        drop(sources);
        drop(read);
        if changes.is_empty() {
            return Ok(report);
        }
        fault!(
            BOUNDARY_BEFORE_ADMISSION,
            Some(self),
            Some(control),
            config_path
        )?;
        let tx = self.db.begin_write()?;
        {
            let sources = tx.open_table(SOURCES)?;
            let hash = |p: &str| -> FResult<Option<String>> {
                sources
                    .get(p)?
                    .map(|r| {
                        crate::store::decode::<SourceMeta>(r.value(), "source").map(|m| m.hash)
                    })
                    .transpose()
            };
            if hash(config_path)?.as_deref() != Some(&config_span.sha256) {
                return Err(FoundryError::Conflict(
                    "boundary config changed during extraction".into(),
                ));
            }
            let mut scopes = tx.open_table(SCOPES)?;
            let mut keys = tx.open_multimap_table(KEYS)?;
            for (path, expected, old, next) in changes {
                control.check()?;
                if hash(&path)? != expected {
                    return Err(FoundryError::Conflict(
                        "boundary source changed during extraction".into(),
                    ));
                }
                let sk = scope_key(config_path, &path);
                // Concurrent publication cannot be overwritten using stale postings.
                let actual: Option<Scope> = scopes
                    .get(sk.as_str())?
                    .map(|r| decode(r.value()))
                    .transpose()?;
                if serde_json::to_string(&actual)? != serde_json::to_string(&old)? {
                    return Err(FoundryError::Conflict(
                        "boundary scope changed during extraction".into(),
                    ));
                }
                if let Some(old) = old {
                    for f in old.facts {
                        if let Some(k) = f.key {
                            keys.remove(posting(config_path, &k).as_str(), path.as_str())?;
                        }
                    }
                }
                if let Some(next) = next {
                    for f in &next.facts {
                        if let Some(k) = &f.key {
                            keys.insert(posting(config_path, k).as_str(), path.as_str())?;
                        }
                    }
                    scopes.insert(sk.as_str(), serde_json::to_string(&next)?.as_str())?;
                } else {
                    scopes.remove(sk.as_str())?;
                }
            }
        }
        fault!(
            BOUNDARY_BEFORE_COMMIT,
            Some(self),
            Some(control),
            config_path
        )?;
        control.check()?;
        tx.commit()?;
        fault!(
            BOUNDARY_AFTER_COMMIT,
            Some(self),
            Some(control),
            config_path
        )?;
        Ok(report)
    }
    pub fn boundary_relations(
        &self,
        config: &str,
        paths: &[String],
        control: &Control,
    ) -> FResult<Relations> {
        self.boundary_relations_in(&self.db.begin_read()?, config, paths, control)
    }
    pub(crate) fn boundary_relations_in(
        &self,
        tx: &redb::ReadTransaction,
        config_path: &str,
        paths: &[String],
        control: &Control,
    ) -> FResult<Relations> {
        if paths.is_empty() || paths.len() > 8 {
            return Err(invalid("boundary traversal requires 1..8 seed paths"));
        }
        for p in paths {
            validate_path(p)?;
        }
        let (config, span) = config_in(tx, config_path)?;
        let sources = tx.open_table(SOURCES)?;
        let scopes = tx.open_table(SCOPES)?;
        let keys = tx.open_multimap_table(KEYS)?;
        let mut current = BTreeMap::<String, Scope>::new();
        let mut incomplete = Vec::new();
        // At most 64 point reads: every configured provider is accounted for,
        // including missing scopes, before any unique join is asserted.
        for path in config.paths() {
            control.check()?;
            let row: Option<Scope> = scopes
                .get(scope_key(config_path, &path).as_str())?
                .map(|r| decode(r.value()))
                .transpose()?;
            let meta: Option<SourceMeta> = sources
                .get(path.as_str())?
                .map(|r| crate::store::decode(r.value(), "source"))
                .transpose()?;
            match (row, meta) {
                (Some(row), Some(meta))
                    if row.recipe == RECIPE
                        && row.config_hash == span.sha256
                        && row.source_hash == meta.hash =>
                {
                    if row.facts.len() > MAX_FACTS
                        || row.facts.iter().any(|f| {
                            !["provide", "consume"].contains(&f.role.as_str())
                                || f.evidence.path != path
                                || f.context.path != path
                                || f.evidence.sha256 != meta.hash
                                || f.context.sha256 != meta.hash
                                || f.evidence.start >= f.evidence.end
                                || f.context.start > f.evidence.start
                                || f.context.end < f.evidence.end
                                || f.context.end > meta.bytes as u64
                                || f.key.is_some() == f.unsupported.is_some()
                                || f.key.as_ref().is_some_and(|k| {
                                    !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]
                                        .contains(&k.method.as_str())
                                        || !k.path.starts_with('/')
                                        || k.path.len() > 2048
                                        || !config.services.iter().any(|s| {
                                            s.id == k.service
                                                && (f.role == "consume"
                                                    || s.providers.contains(&path))
                                        })
                                })
                        })
                    {
                        return Err(FoundryError::GraphInvalid(
                            "boundary scope has invalid facts".into(),
                        ));
                    }
                    if !row.complete {
                        incomplete.push(path.clone());
                    }
                    current.insert(path, row);
                }
                _ => incomplete.push(path),
            }
        }
        let mut seed_keys = BTreeSet::new();
        let mut consumers = BTreeMap::new();
        let mut examined = 0;
        let mut truncated = false;
        for p in paths {
            if let Some(s) = current.get(p) {
                for f in &s.facts {
                    if let Some(k) = &f.key {
                        seed_keys.insert(k.clone());
                    }
                    if f.role == "consume" {
                        consumers.insert((f.evidence.path.clone(), f.evidence.start), f.clone());
                    }
                }
            }
        }
        // Key postings, not pair materialization or a corpus mapping scan.
        for key in seed_keys.iter().take(MAX_FACTS) {
            control.check()?;
            for row in keys.get(posting(config_path, key).as_str())? {
                if examined == 512 {
                    truncated = true;
                    break;
                }
                examined += 1;
                if let Some(scope) = current.get(row?.value()) {
                    for f in &scope.facts {
                        if f.role == "consume" && f.key.as_ref() == Some(key) {
                            consumers
                                .insert((f.evidence.path.clone(), f.evidence.start), f.clone());
                        }
                    }
                }
            }
            if truncated {
                break;
            }
        }
        truncated |= seed_keys.len() > MAX_FACTS || consumers.len() > MAX_RESULTS;
        let mut relations = Vec::new();
        for consumer in consumers.into_values().take(MAX_RESULTS) {
            control.check()?;
            let mut providers = Vec::new();
            let resolution = if let Some(key) = &consumer.key {
                let service = config
                    .services
                    .iter()
                    .find(|s| s.id == key.service)
                    .ok_or_else(|| {
                        FoundryError::GraphInvalid(
                            "boundary key names an unconfigured service".into(),
                        )
                    })?;
                'providers: for path in &service.providers {
                    if let Some(scope) = current.get(path) {
                        for f in scope
                            .facts
                            .iter()
                            .filter(|f| f.role == "provide" && f.key.as_ref() == Some(key))
                        {
                            providers.push(f.clone());
                            if providers.len() == 17 {
                                break 'providers;
                            }
                        }
                    }
                }
                let incomplete = service
                    .providers
                    .iter()
                    .any(|p| !current.get(p).is_some_and(|s| s.complete));
                if providers.len() > 16 {
                    providers.truncate(16);
                    truncated = true;
                }
                if incomplete {
                    "incomplete"
                } else {
                    match providers.len() {
                        0 => "unresolved",
                        1 => "exact_key",
                        _ => "ambiguous",
                    }
                }
            } else {
                "unsupported"
            };
            relations.push(Relation {
                kind: "http_contract",
                resolution,
                consumer,
                providers,
            });
        }
        Ok(Relations {
            recipe: RECIPE,
            evidence_class: "syntactic",
            workspace_id: self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?,
            source_revision: self.freshness_in(tx)?.source_revision,
            config: span,
            scope: "configured_static_python_http_only",
            relations,
            incomplete_paths: incomplete,
            truncated,
            examined,
        })
    }
}

fn text<'a>(n: Node, s: &'a str) -> &'a str {
    &s[n.byte_range()]
}
fn children(n: Node) -> Vec<Node> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}
fn literal(n: Node, s: &str) -> Option<String> {
    if n.kind() != "string" {
        return None;
    }
    let t = text(n, s);
    let quote = t.as_bytes().first().copied()?;
    if ![b'\'', b'"'].contains(&quote) || t.len() < 2 || t.as_bytes().last() != Some(&quote) {
        return None;
    }
    let value = &t[1..t.len() - 1];
    (!value.contains(['\\', '\n', '\r', '\'', '"']) && value.len() <= 2048).then(|| value.into())
}
fn first_argument(n: Node) -> Option<Node> {
    n.child_by_field_name("arguments")?.named_child(0)
}
fn attr(n: Node, s: &str) -> Option<(String, String)> {
    if n.kind() != "attribute" {
        return None;
    }
    let obj = n.child_by_field_name("object")?;
    let a = n.child_by_field_name("attribute")?;
    (obj.kind() == "identifier").then(|| (text(obj, s).into(), text(a, s).into()))
}
fn span(path: &str, hash: &str, n: Node) -> Span {
    Span {
        path: path.into(),
        sha256: hash.into(),
        start: n.start_byte() as u64,
        end: n.end_byte() as u64,
    }
}
fn enclosing(mut n: Node) -> Node {
    let original = n;
    while let Some(p) = n.parent() {
        if p.kind() == "function_definition" || p.kind() == "decorated_definition" {
            return p;
        }
        n = p;
    }
    original
}
fn extract(
    config: &Config,
    path: &str,
    hash: &str,
    source: &str,
    control: &Control,
) -> FResult<(Vec<Fact>, bool)> {
    if source.len() > 1024 * 1024 {
        return Ok((vec![], false));
    }
    control.check()?;
    let tree = match crate::syntax::parse_boundary_python(source) {
        Ok(Some(t)) => t,
        _ => return Ok((vec![], false)),
    };
    control.check()?;
    let root = tree.root_node();
    if root.has_error() {
        return Ok((vec![], false));
    }
    let mut nodes = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if nodes.len() >= 100_000 {
            return Ok((vec![], false));
        }
        if nodes.len() % 256 == 0 {
            control.check()?;
        }
        nodes.push(n);
        stack.extend(children(n).into_iter().rev());
    }
    let mut clients = BTreeSet::new();
    let mut constructors = BTreeSet::new();
    let mut modules = BTreeSet::new();
    let mut approved = BTreeSet::new();
    // Only module-level imports. Conservative binding checks below refuse
    // aliases shadowed anywhere in this source rather than guessing scopes.
    for n in children(root) {
        let t = text(n, source);
        let words: Vec<_> = t.split_whitespace().collect();
        let binding = match words.as_slice() {
            ["import", name] if ["requests", "httpx", "fastapi"].contains(name) => {
                Some((*name, *name))
            }
            ["import", name, "as", alias] if ["requests", "httpx", "fastapi"].contains(name) => {
                Some((*name, *alias))
            }
            ["from", "fastapi", "import", "FastAPI"] => Some(("FastAPI", "FastAPI")),
            ["from", "fastapi", "import", "FastAPI", "as", alias] => Some(("FastAPI", *alias)),
            _ => None,
        };
        if let Some((kind, name)) = binding {
            match kind {
                "FastAPI" => {
                    constructors.insert(name.to_owned());
                }
                "fastapi" => {
                    modules.insert(name.to_owned());
                }
                _ => {
                    clients.insert(name.to_owned());
                }
            }
            approved.insert((n.start_byte(), n.end_byte()));
        }
    }
    let mut apps = BTreeSet::new();
    for n in &nodes {
        if n.kind() != "assignment"
            || n.parent()
                .and_then(|p| p.parent())
                .is_none_or(|p| p.kind() != "module")
        {
            continue;
        }
        let (Some(left), Some(right)) = (
            n.child_by_field_name("left"),
            n.child_by_field_name("right"),
        ) else {
            continue;
        };
        if left.kind() != "identifier" || right.kind() != "call" {
            continue;
        }
        let Some(f) = right.child_by_field_name("function") else {
            continue;
        };
        let valid = constructors.contains(text(f, source))
            || attr(f, source).is_some_and(|(m, a)| modules.contains(&m) && a == "FastAPI");
        // FastAPI constructor options such as root_path are outside this recipe.
        if valid
            && right
                .child_by_field_name("arguments")
                .is_some_and(|a| a.named_child_count() == 0)
        {
            apps.insert(text(left, source).to_owned());
            approved.insert((left.start_byte(), left.end_byte()));
        }
    }
    let names: BTreeSet<_> = clients
        .iter()
        .chain(constructors.iter())
        .chain(modules.iter())
        .chain(apps.iter())
        .cloned()
        .collect();
    let mut unsafe_names = BTreeSet::new();
    for n in &nodes {
        if n.kind() != "identifier" || !names.contains(text(*n, source)) {
            continue;
        }
        if approved
            .range(..=(n.start_byte(), usize::MAX))
            .next_back()
            .is_some_and(|&(_, end)| n.end_byte() <= end)
        {
            continue;
        }
        let receiver = n.parent().is_some_and(|p| {
            p.kind() == "attribute"
                && p.child_by_field_name("object") == Some(*n)
                && p.parent().is_some_and(|call| {
                    call.kind() == "call" && call.child_by_field_name("function") == Some(p)
                })
        });
        let constructor = constructors.contains(text(*n, source))
            && n.parent().is_some_and(|p| {
                p.kind() == "call" && p.child_by_field_name("function") == Some(*n)
            });
        if !receiver && !constructor {
            unsafe_names.insert(text(*n, source).to_owned());
        }
    }
    let binding_incomplete = !unsafe_names.is_empty();
    let service = config
        .services
        .iter()
        .find(|s| s.providers.iter().any(|p| p == path));
    let consumer_admitted = config.consumers.iter().any(|p| p == path);
    let mut facts = Vec::new();
    let mut complete = !binding_incomplete && (service.is_none() || !apps.is_empty());
    for n in nodes {
        if n.kind() != "call" {
            continue;
        }
        let Some((receiver, method)) = n
            .child_by_field_name("function")
            .and_then(|f| attr(f, source))
        else {
            continue;
        };
        let route = apps.contains(&receiver)
            && n.parent().is_some_and(|p| p.kind() == "decorator")
            && service.is_some();
        let consumer = clients.contains(&receiver) && consumer_admitted;
        if !route && !consumer {
            continue;
        }
        if !["get", "post", "put", "patch", "delete", "head", "options"].contains(&method.as_str())
        {
            complete = false;
            continue;
        }
        let value = first_argument(n).and_then(|a| literal(a, source));
        let mut key = None;
        let mut reason = Some("dynamic_or_unsupported_literal".to_string());
        if unsafe_names.contains(&receiver)
            || (route
                && (!unsafe_names.is_disjoint(&constructors)
                    || !unsafe_names.is_disjoint(&modules)))
        {
            reason = Some("shadowed_or_escaped_binding".into());
        } else if let Some(value) = value {
            if route {
                if value.starts_with('/')
                    && !value.contains(['{', '}', '?', '#'])
                    && value.len() <= 1024
                {
                    key = Some(Key {
                        service: service.unwrap().id.clone(),
                        method: method.to_ascii_uppercase(),
                        path: value,
                    });
                }
            } else if let Ok(url) = reqwest::Url::parse(&value)
                && url.username().is_empty()
                && url.password().is_none()
                && url.path().len() <= 2048
                && ["http", "https"].contains(&url.scheme())
            {
                if let Some(s) = config
                    .services
                    .iter()
                    .find(|s| s.origins.contains(&url.origin().ascii_serialization()))
                {
                    key = Some(Key {
                        service: s.id.clone(),
                        method: method.to_ascii_uppercase(),
                        path: url.path().into(),
                    });
                } else {
                    reason = Some("origin_not_configured".into());
                }
            }
        }
        if key.is_some() {
            reason = None;
        }
        if route && reason.is_some() {
            complete = false;
        }
        if facts.len() == MAX_FACTS {
            complete = false;
            break;
        }
        let context = if route {
            n.parent().and_then(|p| p.parent()).unwrap_or(n)
        } else {
            enclosing(n)
        };
        facts.push(Fact {
            role: if route { "provide" } else { "consume" }.into(),
            key,
            evidence: span(path, hash, n),
            context: span(path, hash, context),
            unsupported: reason,
        });
    }
    Ok((facts, complete))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexRequest {
    pub config: String,
    #[serde(default)]
    pub paths: Vec<String>,
}
