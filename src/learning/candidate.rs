//! 013 T002 candidate artifact (contract § Fitting, artifacts and
//! evaluation, 176-189 and 226-237). A candidate directory holds:
//!
//! * `head.safetensors` — exactly the trainable set, float32;
//! * `evaluation.json` — the evaluation report ([`super::eval::Report`]);
//! * `contributions.json` — the inherited and new fitting and calibration
//!   contributions (example identity, input digest, label, permission);
//! * `manifest.json` — written last: schema 4, the recipe, the base
//!   encoder/tokenizer identities and the starting base, the training
//!   record, the ONE fitted temperature, eligibility, and every file's
//!   length and SHA-256.
//!
//! The fitted scalar is the only calibration authority. A manifest that
//! carries any per-option, bucket or per-type temperature (Laya's
//! `temperature_by_options`, its per-type `temperature` array) is refused
//! as `inherited_temperature`; a scalar below 0.5 or off the fitting grid is
//! `temperature_invalid`. Reading a candidate validates everything: the
//! manifest strictly, every file's length and hash, the head's exact
//! tensor set, dtype and finite values, and the report against the
//! manifest. No pickle, no code, no ambient loader lookup.
use super::eval::{self, Report};
use super::{Contribution, OptimizerPin, fail, strict_json};
use crate::control::Control;
use crate::decision_model::{self, safetensors};
use crate::error::FResult;
use crate::neural::anchor::Dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read as _, Seek as _, Write as _};

pub const KIND: &str = "candidate";
pub const HEAD: &str = "head.safetensors";
pub const EVALUATION: &str = "evaluation.json";
pub const CONTRIBUTIONS: &str = "contributions.json";
pub const MANIFEST: &str = "manifest.json";
/// The member files a manifest binds, in name order.
pub const FILES: [&str; 3] = [CONTRIBUTIONS, EVALUATION, HEAD];
const MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
/// Report and contributions for up to 100000 rows stay far below this.
const MEMBER_MAX_BYTES: u64 = 256 * 1024 * 1024;
const COPY_CHUNK: usize = 1 << 20;

/// The starting point of the round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Base {
    /// The pinned initial checkpoint's own head (its tensors only; its
    /// `rl_agent_config.json` temperatures are never read).
    Initial,
    /// An accepted Foundry candidate, by the SHA-256 of its manifest.
    Candidate { manifest_sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncoderIdentity {
    pub checkpoint: decision_model::CheckpointPin,
    /// SHA-256 over the frozen encoder's float32 bytes, identical before and
    /// after training.
    pub frozen_encoder_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenizerIdentity {
    pub json_sha256: String,
    pub config_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Training {
    pub seed: String,
    pub max_steps: u64,
    pub steps_completed: u64,
    /// Epochs begun (the last may be partial).
    pub epochs: u64,
    pub train_rows: usize,
    pub optimizer: OptimizerPin,
    pub head_dropout: f64,
    pub batch: u32,
    pub first_loss: f64,
    pub last_loss: f64,
    /// Updates whose pre-clip gradient norm exceeded the clip.
    pub clipped_steps: u64,
}

/// The save/reload probe: the row whose logits were compared before export
/// and after the worker reloaded the written head.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub example_id: String,
    pub logits: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberFile {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateManifest {
    pub schema: u32,
    pub kind: String,
    pub recipe: String,
    pub workspace_id: String,
    pub model_function_sha256: String,
    pub dataset_manifest_sha256: String,
    pub dataset_id: String,
    pub policy_sha256: String,
    pub base: Base,
    pub encoder: EncoderIdentity,
    pub tokenizer: TokenizerIdentity,
    pub training: Training,
    /// The one fitted scalar (contract 176-189).
    pub temperature: f64,
    pub calibration_rows: usize,
    pub calibration_mean_nll: f64,
    pub evaluation_rows: usize,
    pub eligible: bool,
    pub probe: Probe,
    pub files: Vec<MemberFile>,
}

/// Contribution lineage (contract 153): inherited ones came with the base
/// candidate; new ones are this round's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub new: Vec<Contribution>,
    pub inherited: Vec<Contribution>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributions {
    pub fitting: Lineage,
    pub calibration: Lineage,
}

/// A candidate read back completely.
#[derive(Debug)]
pub struct VerifiedCandidate {
    pub manifest: CandidateManifest,
    pub manifest_sha256: String,
    pub report: Report,
    pub contributions: Contributions,
}

/// Any temperature beside the one top-level scalar, at any depth: a
/// per-option, bucket or per-type override inherited from upstream.
fn inherited_temperature(value: &serde_json::Value, top: bool) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => object.iter().find_map(|(key, child)| {
            let scalar_slot = top && key == "temperature";
            if key.to_ascii_lowercase().contains("temperature") && !scalar_slot {
                return Some(key.clone());
            }
            if scalar_slot && !child.is_number() {
                return Some(format!("{key} (not one scalar)"));
            }
            inherited_temperature(child, false)
        }),
        serde_json::Value::Array(items) => {
            items.iter().find_map(|i| inherited_temperature(i, false))
        }
        _ => None,
    }
}

/// Parse a candidate manifest: temperature rules first (named refusals),
/// then the strict struct (`code` for anything else).
pub fn parse_manifest(bytes: &[u8], code: &'static str) -> FResult<CandidateManifest> {
    let value: serde_json::Value = strict_json(bytes, code, "candidate manifest")?;
    if let Some(key) = inherited_temperature(&value, true) {
        return Err(fail(
            "inherited_temperature",
            format!(
                "the candidate carries an inherited temperature override ({key}); the one \
                 fitted scalar is the only calibration authority"
            ),
        ));
    }
    let manifest: CandidateManifest = serde_json::from_value(value)
        .map_err(|e| fail(code, format!("candidate manifest: {e}")))?;
    if !manifest.temperature.is_finite() || manifest.temperature < 0.5 {
        return Err(fail(
            "temperature_invalid",
            format!(
                "the fitted temperature {} is below 0.5 (or not finite)",
                manifest.temperature
            ),
        ));
    }
    if !eval::valid_temperature(manifest.temperature) {
        return Err(fail(
            "temperature_invalid",
            format!(
                "the fitted temperature {} is not on the k/20 grid (0.5..=3.0)",
                manifest.temperature
            ),
        ));
    }
    if manifest.schema != 4 || manifest.kind != KIND || manifest.recipe != super::RECIPE {
        return Err(fail(
            code,
            "the manifest is not a schema-4 candidate of this recipe",
        ));
    }
    let names: Vec<&str> = manifest.files.iter().map(|f| f.name.as_str()).collect();
    if names != FILES {
        return Err(fail(
            code,
            format!("the manifest must bind exactly {FILES:?} in name order"),
        ));
    }
    Ok(manifest)
}

fn read_member(dir: &Dir, name: &str, cap: u64, code: &'static str) -> FResult<Vec<u8>> {
    let file = dir
        .open_file(name)
        .map_err(|e| fail(code, format!("{name}: {e}")))?;
    super::read_capped(file, cap)
        .map_err(|e| fail(code, format!("{name}: {e}")))?
        .ok_or_else(|| fail(code, format!("{name} exceeds {cap} bytes")))
}

/// Validate a head file through its descriptor: the length, the exact
/// trainable set as float32, every value finite. Returns its SHA-256.
pub fn validate_head(
    mut file: File,
    bytes: u64,
    code: &'static str,
    control: &Control,
) -> FResult<String> {
    let invalid = |message: String| fail(code, format!("{HEAD}: {message}"));
    let length = file.metadata().map_err(|e| invalid(e.to_string()))?.len();
    if length != bytes || length < 8 {
        return Err(invalid(format!("is {length} bytes, expected {bytes}")));
    }
    let mut hasher = Sha256::new();
    let mut prefix = [0u8; 8];
    file.read_exact(&mut prefix)
        .map_err(|e| invalid(e.to_string()))?;
    hasher.update(prefix);
    let header_len = safetensors::header_len(prefix, code)?;
    if header_len > length - 8 {
        return Err(invalid("the header runs past the end".into()));
    }
    let mut header = vec![0u8; header_len as usize];
    file.read_exact(&mut header)
        .map_err(|e| invalid(e.to_string()))?;
    hasher.update(&header);
    let data_len = length - 8 - header_len;
    let entries = safetensors::parse_header(&header, data_len, code)?;
    safetensors::check_tensors(
        &entries,
        &decision_model::trainable(),
        safetensors::Dtype::F32,
        code,
    )?;
    // Every tensor is float32 and they tile the data region, so the data
    // is one run of f32 values.
    let mut reader = std::io::BufReader::with_capacity(COPY_CHUNK, file);
    let mut chunk = vec![0u8; COPY_CHUNK];
    let mut seen = 0u64;
    while seen < data_len {
        let want = COPY_CHUNK.min((data_len - seen) as usize);
        reader
            .read_exact(&mut chunk[..want])
            .map_err(|e| invalid(e.to_string()))?;
        hasher.update(&chunk[..want]);
        if chunk[..want]
            .chunks_exact(4)
            .any(|b| !f32::from_le_bytes([b[0], b[1], b[2], b[3]]).is_finite())
        {
            return Err(fail(
                "nonfinite_weight",
                format!("{HEAD} holds a nonfinite value"),
            ));
        }
        seen += want as u64;
        control.check()?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Stream `name` of `dir` and return its SHA-256 and length, refusing it
/// when the length is not `bytes`.
fn hash_member(dir: &Dir, name: &str, bytes: u64, code: &'static str) -> FResult<String> {
    let mut file = dir
        .open_file(name)
        .map_err(|e| fail(code, format!("{name}: {e}")))?;
    let length = file
        .metadata()
        .map_err(|e| fail(code, format!("{name}: {e}")))?
        .len();
    if length != bytes {
        return Err(fail(
            code,
            format!("{name} is {length} bytes; the manifest says {bytes}"),
        ));
    }
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; COPY_CHUNK];
    loop {
        let n = file
            .read(&mut chunk)
            .map_err(|e| fail(code, format!("{name}: {e}")))?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Read back a whole candidate held as `dir`: the manifest strictly (with
/// the temperature rules), every member's length and SHA-256, the head's
/// tensors and values, and the report and lineage against the manifest.
/// Generic failures carry `code`; temperature refusals keep their own.
pub fn verify_dir(dir: &Dir, code: &'static str, control: &Control) -> FResult<VerifiedCandidate> {
    let raw = read_member(dir, MANIFEST, MANIFEST_MAX_BYTES, code)?;
    let manifest_sha256 = crate::digest(&raw);
    let manifest = parse_manifest(&raw, code)?;
    for member in &manifest.files {
        let sha = if member.name == HEAD {
            let file = dir
                .open_file(HEAD)
                .map_err(|e| fail(code, format!("{HEAD}: {e}")))?;
            validate_head(file, member.bytes, code, control)?
        } else {
            if member.bytes > MEMBER_MAX_BYTES {
                return Err(fail(code, format!("{} is over its bound", member.name)));
            }
            hash_member(dir, &member.name, member.bytes, code)?
        };
        if sha != member.sha256 {
            return Err(fail(
                code,
                format!("{} does not match the manifest's SHA-256", member.name),
            ));
        }
    }
    let report: Report = strict_json(
        &read_member(dir, EVALUATION, MEMBER_MAX_BYTES, code)?,
        code,
        "evaluation report",
    )?;
    let contributions: Contributions = strict_json(
        &read_member(dir, CONTRIBUTIONS, MEMBER_MAX_BYTES, code)?,
        code,
        "contributions",
    )?;
    if report.temperature != manifest.temperature
        || report.eligibility.eligible != manifest.eligible
        || report.rows != manifest.evaluation_rows
    {
        return Err(fail(
            code,
            "the evaluation report disagrees with the manifest",
        ));
    }
    Ok(VerifiedCandidate {
        manifest,
        manifest_sha256,
        report,
        contributions,
    })
}

/// Open and read back the candidate directory at `path` (`--base`,
/// `--incumbent`): a missing directory, a symlink or any invalid member is
/// refused with `code`.
pub fn open(
    path: &std::path::Path,
    code: &'static str,
    control: &Control,
) -> FResult<(Dir, VerifiedCandidate)> {
    let dir = Dir::open_path(path)
        .map_err(|e| fail(code, format!("candidate {}: {e}", path.display())))?;
    let verified = verify_dir(&dir, code, control)?;
    Ok((dir, verified))
}

/// Copy a verified candidate's head into `to` as `name` through both
/// descriptors; returns the copy's SHA-256 (the caller compares it).
pub fn copy_head_into(from: &Dir, to: &Dir, name: &str) -> FResult<String> {
    let io = |what: &str, e: std::io::Error| fail("output_write", format!("{what} {name}: {e}"));
    let mut source = from.open_file(HEAD).map_err(|e| io("open", e))?;
    let mut target = to.create_new(name).map_err(|e| io("create", e))?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; COPY_CHUNK];
    loop {
        let n = source.read(&mut chunk).map_err(|e| io("read", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&chunk[..n]);
        target.write_all(&chunk[..n]).map_err(|e| io("write", e))?;
    }
    target.sync_all().map_err(|e| io("fsync", e))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Which member a candidate write is, for its ENOSPC fault points.
#[derive(Clone, Copy)]
enum Member {
    Head,
    Manifest,
    Other,
}

/// One member write: `write`, `flush` and `fsync`, each passing its fault
/// point first (test builds fail an armed point with `ENOSPC`). Returns the
/// written bytes' SHA-256 and length.
#[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
fn write_member(
    dir: &Dir,
    name: &str,
    member: Member,
    mut source: impl std::io::Read,
    control: &Control,
) -> FResult<MemberFile> {
    let io = |what: &str, e: std::io::Error| fail("output_write", format!("{what} {name}: {e}"));
    let file = dir.create_new(name).map_err(|e| io("create", e))?;
    let mut writer = std::io::BufWriter::with_capacity(COPY_CHUNK, file);
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; COPY_CHUNK];
    let mut bytes = 0u64;
    loop {
        let n = source.read(&mut chunk).map_err(|e| io("read for", e))?;
        if n == 0 {
            break;
        }
        match member {
            Member::Head => learning_io_fault!(HEAD_WRITE, control, name),
            Member::Manifest => learning_io_fault!(MANIFEST_WRITE, control, name),
            Member::Other => Ok(()),
        }
        .and_then(|()| writer.write_all(&chunk[..n]))
        .map_err(|e| io("write", e))?;
        hasher.update(&chunk[..n]);
        bytes += n as u64;
    }
    match member {
        Member::Head => learning_io_fault!(HEAD_FLUSH, control, name),
        Member::Manifest => learning_io_fault!(MANIFEST_FLUSH, control, name),
        Member::Other => Ok(()),
    }
    .and_then(|()| writer.flush())
    .map_err(|e| io("flush", e))?;
    let file = writer
        .into_inner()
        .map_err(|e| io("flush", e.into_error()))?;
    match member {
        Member::Head => learning_io_fault!(HEAD_FSYNC, control, name),
        Member::Manifest => learning_io_fault!(MANIFEST_FSYNC, control, name),
        Member::Other => Ok(()),
    }
    .and_then(|()| file.sync_all())
    .map_err(|e| io("fsync", e))?;
    Ok(MemberFile {
        name: name.to_owned(),
        sha256: format!("{:x}", hasher.finalize()),
        bytes,
    })
}

/// Everything a candidate holds besides its manifest's file list.
pub struct Draft {
    /// The validated head in the worker's scratch, held by descriptor.
    pub head: File,
    pub head_sha256: String,
    pub report: Report,
    pub contributions: Contributions,
    /// The manifest with an empty `files` list; [`write`] fills it.
    pub manifest: CandidateManifest,
}

/// Compact JSON of the report and the lineage.
fn encode<T: Serialize>(value: &T) -> FResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(crate::FoundryError::from)
}

/// Write the candidate into the owned partial directory: the members, then
/// the manifest LAST, each written, flushed and fsynced, then the directory
/// fsynced. The head is copied from the held scratch descriptor and must
/// hash to the bytes validated there. The manifest is built (it binds every
/// member's length and SHA-256, all known before writing) and the WHOLE
/// candidate, manifest included, is held to `output_bytes` before anything
/// is written (`output_limit`, nothing written). Returns the manifest bytes.
pub fn write(
    partial: &Dir,
    mut draft: Draft,
    output_bytes: u64,
    control: &Control,
) -> FResult<Vec<u8>> {
    let evaluation = encode(&draft.report)?;
    let contributions = encode(&draft.contributions)?;
    let head_bytes = draft
        .head
        .metadata()
        .map_err(|e| fail("output_write", format!("{HEAD}: {e}")))?
        .len();
    let entry = |name: &str, bytes: &[u8]| MemberFile {
        name: name.to_owned(),
        sha256: crate::digest(bytes),
        bytes: bytes.len() as u64,
    };
    let expected = vec![
        entry(CONTRIBUTIONS, &contributions),
        entry(EVALUATION, &evaluation),
        MemberFile {
            name: HEAD.to_owned(),
            sha256: draft.head_sha256.clone(),
            bytes: head_bytes,
        },
    ];
    draft.manifest.files = expected.clone();
    let manifest = encode(&draft.manifest)?;
    if manifest.len() as u64 > MANIFEST_MAX_BYTES {
        return Err(fail("output_limit", "the candidate manifest is over 1 MiB"));
    }
    let total = expected
        .iter()
        .map(|member| member.bytes)
        .sum::<u64>()
        .saturating_add(manifest.len() as u64);
    if total > output_bytes {
        return Err(fail(
            "output_limit",
            format!(
                "the candidate would be {total} bytes with its manifest; the output ceiling is \
                 {output_bytes}"
            ),
        ));
    }
    draft
        .head
        .rewind()
        .map_err(|e| fail("output_write", format!("{HEAD}: {e}")))?;
    let written = [
        write_member(
            partial,
            CONTRIBUTIONS,
            Member::Other,
            contributions.as_slice(),
            control,
        )?,
        write_member(
            partial,
            EVALUATION,
            Member::Other,
            evaluation.as_slice(),
            control,
        )?,
        write_member(partial, HEAD, Member::Head, &mut draft.head, control)?,
    ];
    if written[..] != expected[..] {
        return Err(fail(
            "output_ownership",
            "a member changed between its validation and its copy",
        ));
    }
    write_member(
        partial,
        MANIFEST,
        Member::Manifest,
        manifest.as_slice(),
        control,
    )?;
    partial
        .sync_all()
        .map_err(|e| fail("output_write", format!("fsync the candidate: {e}")))?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_temperature_beside_the_one_scalar_is_inherited() {
        let value = serde_json::json!({"temperature": 1.0, "x": {"temperature_by_options": {}}});
        assert!(inherited_temperature(&value, true).is_some());
        let value = serde_json::json!({"temperature": [1.0, 1.1, 1.2]});
        assert!(inherited_temperature(&value, true).is_some());
        let value = serde_json::json!({"temperature": 1.05, "training": {"seed": "s"}});
        assert!(inherited_temperature(&value, true).is_none());
        let value = serde_json::json!({"probe": {"Temperature": 2.0}});
        assert!(inherited_temperature(&value, true).is_some());
    }

    fn draft(dir: &std::path::Path) -> Draft {
        let head = dir.join("head-source");
        std::fs::write(&head, b"0123456789abcdef").unwrap();
        let cases = vec![eval::CaseInput {
            example_id: "e".into(),
            group_id: "g".into(),
            option_ids: ["search".into(), "graph".into()],
            expected: "search".into(),
            baseline: "search".into(),
        }];
        let report = eval::evaluate(
            &cases,
            &[eval::Output::Logits([1.0, 0.0])],
            1.0,
            None,
            &super::super::SelectionPolicy::default(),
        );
        let pin = decision_model::CheckpointPin {
            weights_sha256: "1".repeat(64),
            encoder_config_sha256: "2".repeat(64),
            source_dtype: "F16".into(),
        };
        Draft {
            head: File::open(&head).unwrap(),
            head_sha256: crate::digest(b"0123456789abcdef"),
            report,
            contributions: Contributions::default(),
            manifest: CandidateManifest {
                schema: 4,
                kind: KIND.into(),
                recipe: super::super::RECIPE.into(),
                workspace_id: "w".into(),
                model_function_sha256: "f".repeat(64),
                dataset_manifest_sha256: "d".repeat(64),
                dataset_id: "i".repeat(64),
                policy_sha256: "p".repeat(64),
                base: Base::Initial,
                encoder: EncoderIdentity {
                    checkpoint: pin,
                    frozen_encoder_sha256: "3".repeat(64),
                },
                tokenizer: TokenizerIdentity {
                    json_sha256: "4".repeat(64),
                    config_sha256: "5".repeat(64),
                },
                training: Training {
                    seed: "s".into(),
                    max_steps: 1,
                    steps_completed: 1,
                    epochs: 1,
                    train_rows: 1,
                    optimizer: OptimizerPin::recipe(),
                    head_dropout: 0.1,
                    batch: 1,
                    first_loss: 0.5,
                    last_loss: 0.5,
                    clipped_steps: 0,
                },
                temperature: 1.0,
                calibration_rows: 1,
                calibration_mean_nll: 0.5,
                evaluation_rows: 1,
                eligible: false,
                probe: Probe {
                    example_id: "e".into(),
                    logits: [1.0, 0.0],
                },
                files: Vec::new(),
            },
        }
    }

    /// Bytes of every file in `dir`.
    fn written(dir: &std::path::Path) -> (usize, u64) {
        let mut count = 0;
        let mut total = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            count += 1;
            total += entry.unwrap().metadata().unwrap().len();
        }
        (count, total)
    }

    #[test]
    fn the_output_ceiling_counts_the_manifest_and_refuses_one_byte_over() {
        let work = tempfile::tempdir().unwrap();
        let control = Control::unbounded();
        let attempt = |name: &str, ceiling: u64| {
            let out = work.path().join(name);
            std::fs::create_dir(&out).unwrap();
            let dir = Dir::open_path(&out).unwrap();
            (write(&dir, draft(work.path()), ceiling, &control), out)
        };
        // Measure the whole candidate once.
        let (result, out) = attempt("measure", u64::MAX);
        let manifest = result.unwrap();
        let (files, total) = written(&out);
        assert_eq!(files, 4);
        assert!(total > manifest.len() as u64);
        // Exactly the total, manifest included, is admitted.
        let (result, out) = attempt("exact", total);
        assert_eq!(result.unwrap(), manifest);
        assert_eq!(written(&out), (4, total));
        // One byte less is refused before anything is written, even though
        // the members alone would fit.
        let members = total - manifest.len() as u64;
        assert!(members < total - 1);
        let (result, out) = attempt("over", total - 1);
        assert_eq!(result.unwrap_err().code(), "output_limit");
        assert_eq!(written(&out), (0, 0), "nothing was written");
    }
}
