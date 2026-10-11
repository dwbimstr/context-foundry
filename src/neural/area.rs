//! Independent vectors of current multi-source briefs. Source vectors keep the
//! ANN owner; this small overview layer uses a bounded exact cosine scan and the
//! same durable vector cache. It never promotes generated claims to source facts.
use super::{cache, partition::TokenCount, profile::SemanticProfile, provider};
use crate::{Control, Engine, FResult, FoundryError, brief};
use redb::{ReadableDatabase, ReadableTable};
use serde::{Deserialize, Serialize};

const MAX_BRIEFS: usize = 4096;
pub(crate) const MAX_AREAS: usize = 256;
const PREFIX: &str = "area-v1:";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    id: String,
    revision: u64,
    fingerprint: String,
    digest: String,
    recipe: String,
    key: String,
}

pub(crate) struct Input {
    pub record: Record,
    pub ids: Vec<u32>,
}
impl Input {
    pub fn key(&self) -> &str {
        &self.record.key
    }
}

fn prefix(profile: &SemanticProfile) -> String {
    format!(
        "{PREFIX}{}:",
        crate::digest(
            format!("{}:{}", profile.descriptor.digest(), profile.recipe_id()).as_bytes()
        )
    )
}
fn enabled(profile: &SemanticProfile) -> bool {
    matches!(
        profile.representation,
        super::partition::Representation::Hierarchy { .. }
    )
}

/// Snapshot bounded records, close the transaction, then fit exact model inputs.
/// All dependencies and lineage are checked again when binding the cached result.
pub(crate) fn inputs(
    engine: &Engine,
    profile: &SemanticProfile,
    tokens: &dyn TokenCount,
    control: &Control,
) -> FResult<Vec<Input>> {
    if !enabled(profile) {
        return Ok(Vec::new());
    }
    let records = snapshot(engine, profile, control)?;
    render(records, profile, tokens, control)
}

pub(crate) fn snapshot(
    engine: &Engine,
    profile: &SemanticProfile,
    control: &Control,
) -> FResult<Vec<brief::Record>> {
    if !enabled(profile) {
        return Ok(Vec::new());
    }
    let tx = engine.db.begin_read()?;
    let briefs = tx.open_table(brief::BRIEFS)?;
    let sources = tx.open_table(crate::store::SOURCES)?;
    let mut records = Vec::new();
    for (n, row) in briefs.iter()?.enumerate() {
        control.check()?;
        if n >= MAX_BRIEFS {
            return Err(limit("area preparation exceeds 4096 briefs"));
        }
        let (id, raw) = row?;
        let record = brief::decode_row(id.value(), raw.value())?;
        if record.dependencies.len() < 2 || !brief::eligible(&record, &sources, &briefs)? {
            continue;
        }
        if records.len() >= MAX_AREAS {
            return Err(limit(
                "area preparation exceeds 256 current multi-source briefs",
            ));
        }
        records.push(record);
    }
    Ok(records)
}

pub(crate) fn cached(engine: &Engine, profile: &SemanticProfile, input: &Input) -> FResult<bool> {
    Ok(matches!(
        engine.semantic_cache_lookup(
            input.key(),
            &profile.descriptor.digest(),
            profile.descriptor.dims()
        )?,
        cache::CacheLookup::Hit(_)
    ))
}

pub(crate) fn render(
    records: Vec<brief::Record>,
    profile: &SemanticProfile,
    tokens: &dyn TokenCount,
    control: &Control,
) -> FResult<Vec<Input>> {
    let mut inputs = Vec::new();
    for record in records {
        control.check()?;
        let mut text =
            "area-v1\nderived multi-source overview (claims require source verification):\n"
                .to_owned();
        let claims = [
            (&record.brief.purpose, "purpose"),
            (&record.brief.behavior, "behavior"),
            (&record.brief.provides, "provides"),
            (&record.brief.consumes, "consumes"),
            (&record.brief.constraints, "constraints"),
            (&record.brief.tests, "tests"),
        ];
        let mut admitted = false;
        for (claims, name) in claims {
            for claim in claims {
                let candidate = format!("{text}{name}: {}\n", claim.text);
                if tokens.count(&provider::render(
                    &profile.descriptor.document_template,
                    &candidate,
                ))? <= profile.card_tokens as usize
                {
                    text = candidate;
                    admitted = true;
                }
            }
        }
        // A shared framing string without any claim carries no area semantics.
        // Keep this brief available to lexical discovery, without a vector.
        if !admitted {
            continue;
        }
        let rendered = provider::render(&profile.descriptor.document_template, &text);
        let ids = tokens.encode(&rendered)?.ids;
        if ids.is_empty() || ids.len() > profile.card_tokens as usize {
            return Err(provider::ProviderError::InputTooLarge(
                "area prefix exceeds the card limit".into(),
            )
            .into());
        }
        let digest = profile.descriptor.digest();
        let key = provider::input_key(&digest, &rendered);
        inputs.push(Input {
            record: Record {
                id: record.id.clone(),
                revision: record.revision,
                fingerprint: crate::knowledge::fingerprint(&record)?,
                digest,
                recipe: profile.recipe_id(),
                key,
            },
            ids,
        });
    }
    Ok(inputs)
}

fn limit(message: &str) -> FoundryError {
    FoundryError::Semantic {
        code: "area_limit",
        message: message.into(),
    }
}

pub(crate) fn prune(
    engine: &Engine,
    profile: &SemanticProfile,
    inputs: &[Input],
    control: &Control,
) -> FResult<()> {
    if !enabled(profile) {
        return Ok(());
    }
    let prefix = prefix(profile);
    let ids: std::collections::BTreeSet<_> = inputs
        .iter()
        .map(|input| input.record.id.as_str())
        .collect();
    let tx = engine.db.begin_write()?;
    let mut table = tx.open_table(cache::STATE)?;
    let upper = format!("{prefix}~");
    let mut remove = Vec::new();
    for row in table.range(prefix.as_str()..upper.as_str())? {
        control.check()?;
        let (key, _) = row?;
        if !ids.contains(key.value().strip_prefix(&prefix).unwrap_or("")) {
            remove.push(key.value().to_owned());
        }
    }
    if remove.is_empty() {
        return Ok(());
    }
    for key in remove {
        table.remove(key.as_str())?;
    }
    drop(table);
    control.check()?;
    tx.commit()?;
    Ok(())
}

/// Bind only a still-current brief version. A late vector may stay cached, but
/// never makes a changed/deleted area eligible. No source or brief is rewritten.
pub(crate) fn bind(
    engine: &Engine,
    profile: &SemanticProfile,
    record: &Record,
    control: &Control,
) -> FResult<bool> {
    control.check()?;
    let tx = engine.db.begin_write()?;
    let briefs = tx.open_table(brief::BRIEFS)?;
    let Some(raw) = briefs.get(record.id.as_str())? else {
        return Ok(false);
    };
    let current = brief::decode_row(&record.id, raw.value())?;
    if crate::knowledge::fingerprint(&current)? != record.fingerprint
        || !brief::eligible(&current, &tx.open_table(crate::store::SOURCES)?, &briefs)?
    {
        return Ok(false);
    }
    let key = format!("{}{}", prefix(profile), record.id);
    let encoded = serde_json::to_string(record)?;
    {
        let mut state = tx.open_table(cache::STATE)?;
        if state
            .get(key.as_str())?
            .is_some_and(|raw| raw.value() == encoded)
        {
            return Ok(true);
        }
        state.insert(key.as_str(), encoded.as_str())?;
    }
    drop(raw);
    drop(briefs);
    control.check()?;
    tx.commit()?;
    Ok(true)
}

/// Read at most 256 overview bindings; the caller merges these into ordinary
/// brief discovery, whose final read rechecks the brief revision and every source.
pub(crate) fn search(
    engine: &Engine,
    profile: &SemanticProfile,
    query: &[f32],
    control: &Control,
) -> FResult<Vec<crate::store::RecordCandidate>> {
    if !enabled(profile) {
        return Ok(Vec::new());
    }
    let tx = engine.db.begin_read()?;
    let state = tx.open_table(cache::STATE)?;
    let briefs = tx.open_table(brief::BRIEFS)?;
    let sources = tx.open_table(crate::store::SOURCES)?;
    let prefix = prefix(profile);
    let upper = format!("{prefix}~");
    let mut candidates = Vec::new();
    for row in state
        .range(prefix.as_str()..upper.as_str())?
        .take(MAX_AREAS)
    {
        control.check()?;
        let (_, raw) = row?;
        let record: Record = serde_json::from_str(raw.value())
            .map_err(|_| FoundryError::CorruptStore("invalid area vector binding".into()))?;
        if record.digest != profile.descriptor.digest() || record.recipe != profile.recipe_id() {
            continue;
        }
        let Some(raw) = briefs.get(record.id.as_str())? else {
            continue;
        };
        let brief = brief::decode_row(&record.id, raw.value())?;
        if crate::knowledge::fingerprint(&brief)? != record.fingerprint
            || !brief::eligible(&brief, &sources, &briefs)?
        {
            continue;
        }
        let cache::CacheLookup::Hit(vector) =
            engine.semantic_cache_lookup(&record.key, &record.digest, query.len())?
        else {
            continue;
        };
        let score: f32 = vector.iter().zip(query).map(|(a, b)| a * b).sum();
        if score.is_finite() {
            candidates.push((score, record.id, record.revision));
        }
    }
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(candidates
        .into_iter()
        .take(brief::CONTEXT_HITS)
        .map(|(_, id, revision)| crate::store::RecordCandidate { id, revision })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn independent_area_cache_reuses_text_and_rejects_changed_dependencies_and_revisions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        let wid = engine.workspace_id().unwrap();
        let a = "fn x() { refund(); }\n";
        let b = "fn test_x() { x(); }\n";
        engine.replace_source("a.rs", a).unwrap();
        engine.replace_source("b.rs", b).unwrap();
        let handle = |path: &str, body: &str| {
            format!(
                "{path}#0-{}@{}.{}",
                body.len(),
                &crate::digest(body.as_bytes())[..32],
                &wid[..16]
            )
        };
        let input = |second: &str| {
            json!({"cited_spans":[handle("a.rs",a),handle("b.rs",second)],
            "recipe_sha256":"a".repeat(64),"model":"fixture","artifact_sha256":"b".repeat(64),
            "prompt_sha256":"c".repeat(64),"generated_at_unix_ms":1,
            "purpose":[{"text":"Refund reimbursement credit","citations":[0],"identifiers":[]}],
            "behavior":[{"text":"Invoke refund through x.","citations":[0],"identifiers":[]}],"provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":[]})
        };
        let put = |second: &str, expected: Option<u64>| {
            let request = brief::parse(
                &serde_json::to_vec(
                    &json!({"workspace_id":wid,"command":{"op":"put","brief":input(second),"expected_revision":expected}}),
                )
                .unwrap(),
            )
            .unwrap();
            engine.brief(&request, &Control::unbounded()).unwrap()
        };
        let initial = put(b, None);
        let path = crate::testkit::write_semantic_profile(dir.path(), "area", |_| {});
        let mut profile = SemanticProfile::load(&path).unwrap();
        profile.representation =
            super::super::partition::Representation::Hierarchy { parent_tokens: 32 };
        profile.card_tokens = 512;
        let tokenizer = super::super::tokenize::DocumentTokenizer::load(&profile).unwrap();
        let control = Control::unbounded();
        let first = inputs(&engine, &profile, &tokenizer, &control).unwrap();
        assert_eq!(first.len(), 1);
        let mut oversized = snapshot(&engine, &profile, &control).unwrap();
        oversized[0].brief.purpose[0].text = "oversized ".repeat(1024);
        oversized[0].brief.behavior[0].text = "oversized ".repeat(1024);
        assert!(
            render(oversized, &profile, &tokenizer, &control)
                .unwrap()
                .is_empty()
        );
        assert!(!cached(&engine, &profile, &first[0]).unwrap());
        let mut vector = vec![0.; profile.descriptor.dims()];
        vector[0] = 1.;
        engine
            .semantic_cache_commit(
                &[(first[0].key().to_owned(), vector.clone())],
                &profile.descriptor.digest(),
                profile.descriptor.dims(),
                cache::DEFAULT_CACHE_CAP_BYTES,
            )
            .unwrap();
        assert!(bind(&engine, &profile, &first[0].record, &control).unwrap());
        assert_eq!(
            search(&engine, &profile, &vector, &control).unwrap().len(),
            1
        );
        assert!(
            cached(
                &engine,
                &profile,
                &inputs(&engine, &profile, &tokenizer, &control).unwrap()[0]
            )
            .unwrap()
        );
        let changed = "fn test_x() { x(); x(); }\n";
        engine.replace_source("b.rs", changed).unwrap();
        assert!(
            search(&engine, &profile, &vector, &control)
                .unwrap()
                .is_empty()
        );
        assert!(!bind(&engine, &profile, &first[0].record, &control).unwrap());
        put(changed, initial["revision"].as_u64());
        let next = inputs(&engine, &profile, &tokenizer, &control).unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(
            next[0].key(),
            first[0].key(),
            "same generated prose reuses its vector"
        );
        assert!(cached(&engine, &profile, &next[0]).unwrap());
        prune(&engine, &profile, &next, &control).unwrap();
        assert!(bind(&engine, &profile, &next[0].record, &control).unwrap());
        assert_eq!(
            search(&engine, &profile, &vector, &control).unwrap().len(),
            1
        );
        engine.delete_source("a.rs").unwrap();
        assert!(
            search(&engine, &profile, &vector, &control)
                .unwrap()
                .is_empty()
        );
    }
}
