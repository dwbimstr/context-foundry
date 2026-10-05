//! Usage receipts and offline summary (003 adapter-economics contract v1).
//!
//! Receipts are attributed observations of actual provider responses, never
//! training permission or source truth. They carry counters and identities
//! only: no raw prompts, source, keys or labels. Key is the TUPLE
//! `(adapter_id, session_id, request_id)`; an identical retry counts once and
//! changed values under the same key are `receipt_conflict`. Logging is a
//! bounded opt-in private JSONL file; `foundry usage summarize --input FILE`
//! reads at most 16 MiB / 10,000 rows, refuses excess rather than truncating,
//! and opens no store or provider connection. Totals use checked u64
//! arithmetic: an unrepresentable total is a named refusal, never a wrapped,
//! saturated or panicking value.

use std::{
    collections::{BTreeMap, HashMap},
    io::{Read as _, Write as _},
    path::Path,
};

use serde_json::{Map, Value};

use crate::{
    FoundryError,
    adapter_error::{AResult, AdapterError},
};

pub const RECEIPT_MAX_BYTES: usize = 16 * 1024;
pub const SUMMARIZE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const SUMMARIZE_MAX_ROWS: usize = 10_000;
pub const CONTEXT_IDS_MAX: usize = 64;

fn invalid(detail: &str) -> FoundryError {
    FoundryError::InvalidArgument(format!("receipt: {detail}"))
}

fn is_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

/// Delivery IDs are fresh random UUIDv4 values in canonical lowercase form.
fn is_delivery_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .map(|id| id.get_version_num() == 4 && id.hyphenated().to_string() == value)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Complete,
    Failed,
    Unknown,
}

impl Outcome {
    fn as_str(&self) -> &'static str {
        match self {
            Outcome::Complete => "complete",
            Outcome::Failed => "failed",
            Outcome::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "complete" => Some(Outcome::Complete),
            "failed" => Some(Outcome::Failed),
            "unknown" => Some(Outcome::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Delivery,
    HostRequest,
    Meter,
    Enforce,
}

impl Mode {
    fn as_str(&self) -> &'static str {
        match self {
            Mode::Delivery => "delivery",
            Mode::HostRequest => "host_request",
            Mode::Meter => "meter",
            Mode::Enforce => "enforce",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "delivery" => Some(Mode::Delivery),
            "host_request" => Some(Mode::HostRequest),
            "meter" => Some(Mode::Meter),
            "enforce" => Some(Mode::Enforce),
            _ => None,
        }
    }
}

/// Receipt v1. `input_tokens` is normalized to include cached input;
/// `cached_input_tokens` must not exceed `input_tokens`. Cost is a mutually
/// present triple; `ratecard_id` is allowed only for calculated cost and
/// required there.
#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    pub session_id: String,
    pub request_id: String,
    pub adapter_id: String,
    pub model_id: String,
    pub context_ids: Vec<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
    pub currency: Option<String>,
    pub cost_basis: Option<String>,
    pub ratecard_id: Option<String>,
    pub outcome: Outcome,
    pub observation: Option<Observation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub mode: Mode,
    pub elapsed_ms: u64,
    pub count_elapsed_ms: Option<u64>,
    pub provider_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    /// Gateway-only: whether the response reached the host, recorded
    /// separately from `outcome` (a closed client never erases known counts).
    pub delivery: Option<Delivery>,
}

/// How a gateway response was delivered to the host (gateway-only field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Delivered,
    ClientClosed,
    LocalFailure,
}

impl Delivery {
    fn as_str(&self) -> &'static str {
        match self {
            Delivery::Delivered => "delivered",
            Delivery::ClientClosed => "client_closed",
            Delivery::LocalFailure => "local_failure",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "delivered" => Some(Delivery::Delivered),
            "client_closed" => Some(Delivery::ClientClosed),
            "local_failure" => Some(Delivery::LocalFailure),
            _ => None,
        }
    }
}

/// Optional means omittable, not null: an explicit null refuses.
fn optional<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, FoundryError> {
    match object.get(name) {
        None => Ok(None),
        Some(Value::Null) => Err(invalid(&format!("`{name}` must be omitted, not null"))),
        Some(value) => Ok(Some(value)),
    }
}

fn id_field(object: &Map<String, Value>, name: &str) -> Result<String, FoundryError> {
    match object.get(name) {
        Some(Value::String(s)) if is_id(s) => Ok(s.clone()),
        _ => Err(invalid(&format!(
            "`{name}` must be a nonblank string of at most 256 UTF-8 bytes"
        ))),
    }
}

fn optional_id(object: &Map<String, Value>, name: &str) -> Result<Option<String>, FoundryError> {
    match optional(object, name)? {
        None => Ok(None),
        Some(Value::String(s)) if is_id(s) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(&format!(
            "`{name}` must be a nonblank string of at most 256 UTF-8 bytes"
        ))),
    }
}

fn count_field(object: &Map<String, Value>, name: &str) -> Result<Option<u64>, FoundryError> {
    match optional(object, name)? {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid(&format!("`{name}` must be a nonnegative integer"))),
    }
}

const RECEIPT_FIELDS: [&str; 15] = [
    "v",
    "session_id",
    "request_id",
    "adapter_id",
    "model_id",
    "context_ids",
    "input_tokens",
    "output_tokens",
    "cached_input_tokens",
    "cost_microunits",
    "currency",
    "cost_basis",
    "ratecard_id",
    "outcome",
    "observation",
];

const OBSERVATION_FIELDS: [&str; 6] = [
    "mode",
    "elapsed_ms",
    "count_elapsed_ms",
    "provider_request_id",
    "provider_response_id",
    "delivery",
];

impl Receipt {
    /// Strict parse: unknown fields and explicit nulls refuse; the serialized
    /// receipt must fit 16 KiB.
    pub fn parse(bytes: &[u8]) -> Result<Self, FoundryError> {
        if bytes.len() > RECEIPT_MAX_BYTES {
            return Err(invalid("receipt exceeds 16 KiB"));
        }
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(&format!("invalid JSON: {e}")))?;
        let Some(object) = value.as_object() else {
            return Err(invalid("receipt must be a JSON object"));
        };
        for key in object.keys() {
            if !RECEIPT_FIELDS.contains(&key.as_str()) {
                return Err(invalid(&format!("unknown field `{key}`")));
            }
        }
        if object.get("v").and_then(Value::as_u64) != Some(1) {
            return Err(invalid("`v` must be 1"));
        }
        let session_id = id_field(object, "session_id")?;
        let request_id = id_field(object, "request_id")?;
        let adapter_id = id_field(object, "adapter_id")?;
        let model_id = id_field(object, "model_id")?;
        let context_ids = match optional(object, "context_ids")? {
            None => Vec::new(),
            Some(Value::Array(items)) => {
                if items.len() > CONTEXT_IDS_MAX {
                    return Err(invalid("`context_ids` allows at most 64 entries"));
                }
                let mut ids = Vec::new();
                for item in items {
                    let Some(text) = item.as_str() else {
                        return Err(invalid("`context_ids` entries must be strings"));
                    };
                    if !is_delivery_uuid(text) {
                        return Err(invalid(
                            "`context_ids` entries must be lowercase delivery UUIDv4 values",
                        ));
                    }
                    ids.push(text.to_owned());
                }
                let mut distinct = ids.clone();
                distinct.sort();
                distinct.dedup();
                if distinct.len() != ids.len() {
                    return Err(invalid("`context_ids` must be distinct"));
                }
                ids
            }
            Some(_) => return Err(invalid("`context_ids` must be an array")),
        };
        let input_tokens = count_field(object, "input_tokens")?;
        let output_tokens = count_field(object, "output_tokens")?;
        let cached_input_tokens = count_field(object, "cached_input_tokens")?;
        if let (Some(cached), Some(input)) = (cached_input_tokens, input_tokens)
            && cached > input
        {
            return Err(invalid(
                "`cached_input_tokens` must not exceed `input_tokens`",
            ));
        }
        let cost_microunits = count_field(object, "cost_microunits")?;
        let currency = match optional(object, "currency")? {
            None => None,
            Some(Value::String(s)) => {
                let bytes = s.as_bytes();
                if bytes.len() == 3 && bytes.iter().all(u8::is_ascii_uppercase) {
                    Some(s.clone())
                } else {
                    return Err(invalid("`currency` must be three uppercase ASCII letters"));
                }
            }
            Some(_) => return Err(invalid("`currency` must be a string")),
        };
        let cost_basis = match optional(object, "cost_basis")? {
            None => None,
            Some(Value::String(s)) if s == "reported" || s == "calculated" => Some(s.clone()),
            Some(_) => return Err(invalid("`cost_basis` must be `reported` or `calculated`")),
        };
        let ratecard_id = optional_id(object, "ratecard_id")?;
        // Cost is a mutually present triple; no partial triples.
        let cost_present = [
            cost_microunits.is_some(),
            currency.is_some(),
            cost_basis.is_some(),
        ];
        if cost_present.iter().any(|p| *p) && !cost_present.iter().all(|p| *p) {
            return Err(invalid(
                "cost requires the full triple: cost_microunits, currency, cost_basis",
            ));
        }
        match cost_basis.as_deref() {
            Some("calculated") if ratecard_id.is_none() => {
                return Err(invalid("`ratecard_id` is required for calculated cost"));
            }
            Some("reported") if ratecard_id.is_some() => {
                return Err(invalid("`ratecard_id` is only allowed for calculated cost"));
            }
            None if ratecard_id.is_some() => {
                return Err(invalid("`ratecard_id` without a cost triple is invalid"));
            }
            _ => {}
        }
        let outcome = match object.get("outcome") {
            Some(Value::String(s)) => Outcome::parse(s)
                .ok_or_else(|| invalid("`outcome` must be complete|failed|unknown"))?,
            _ => return Err(invalid("`outcome` is required")),
        };
        let observation = match optional(object, "observation")? {
            None => None,
            Some(Value::Object(fields)) => {
                for key in fields.keys() {
                    if !OBSERVATION_FIELDS.contains(&key.as_str()) {
                        return Err(invalid(&format!("unknown observation field `{key}`")));
                    }
                }
                let mode = match fields.get("mode") {
                    Some(Value::String(s)) => {
                        Mode::parse(s).ok_or_else(|| invalid("observation mode is invalid"))?
                    }
                    _ => return Err(invalid("observation `mode` is required")),
                };
                let elapsed_ms = fields
                    .get("elapsed_ms")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("observation `elapsed_ms` is required"))?;
                Some(Observation {
                    mode,
                    elapsed_ms,
                    count_elapsed_ms: count_field(fields, "count_elapsed_ms")?,
                    provider_request_id: optional_id(fields, "provider_request_id")?,
                    provider_response_id: optional_id(fields, "provider_response_id")?,
                    delivery: match optional(fields, "delivery")? {
                        None => None,
                        Some(Value::String(s)) => Some(Delivery::parse(s).ok_or_else(|| {
                            invalid("`delivery` must be delivered|client_closed|local_failure")
                        })?),
                        Some(_) => {
                            return Err(invalid(
                                "`delivery` must be delivered|client_closed|local_failure",
                            ));
                        }
                    },
                })
            }
            Some(_) => return Err(invalid("`observation` must be an object")),
        };
        Ok(Receipt {
            session_id,
            request_id,
            adapter_id,
            model_id,
            context_ids,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cost_microunits,
            currency,
            cost_basis,
            ratecard_id,
            outcome,
            observation,
        })
    }

    /// The identity is the tuple itself: no delimiter concatenation can make
    /// two distinct tuples collide.
    pub fn key(&self) -> (String, String, String) {
        (
            self.adapter_id.clone(),
            self.session_id.clone(),
            self.request_id.clone(),
        )
    }

    /// Absent fields are omitted, never serialized as null, so every emitted
    /// line parses back under the strict reader.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("v".into(), Value::from(1));
        map.insert("session_id".into(), Value::from(self.session_id.clone()));
        map.insert("request_id".into(), Value::from(self.request_id.clone()));
        map.insert("adapter_id".into(), Value::from(self.adapter_id.clone()));
        map.insert("model_id".into(), Value::from(self.model_id.clone()));
        if !self.context_ids.is_empty() {
            map.insert("context_ids".into(), Value::from(self.context_ids.clone()));
        }
        for (name, value) in [
            ("input_tokens", self.input_tokens),
            ("output_tokens", self.output_tokens),
            ("cached_input_tokens", self.cached_input_tokens),
            ("cost_microunits", self.cost_microunits),
        ] {
            if let Some(value) = value {
                map.insert(name.into(), Value::from(value));
            }
        }
        for (name, value) in [
            ("currency", &self.currency),
            ("cost_basis", &self.cost_basis),
            ("ratecard_id", &self.ratecard_id),
        ] {
            if let Some(value) = value {
                map.insert(name.into(), Value::from(value.clone()));
            }
        }
        map.insert("outcome".into(), Value::from(self.outcome.as_str()));
        if let Some(observation) = &self.observation {
            let mut fields = Map::new();
            fields.insert("mode".into(), Value::from(observation.mode.as_str()));
            fields.insert("elapsed_ms".into(), Value::from(observation.elapsed_ms));
            if let Some(value) = observation.count_elapsed_ms {
                fields.insert("count_elapsed_ms".into(), Value::from(value));
            }
            if let Some(value) = &observation.provider_request_id {
                fields.insert("provider_request_id".into(), Value::from(value.clone()));
            }
            if let Some(value) = &observation.provider_response_id {
                fields.insert("provider_response_id".into(), Value::from(value.clone()));
            }
            if let Some(value) = observation.delivery {
                fields.insert("delivery".into(), Value::from(value.as_str()));
            }
            map.insert("observation".into(), Value::Object(fields));
        }
        Value::Object(map)
    }
}

/// In-memory deduplication: an identical retry counts once; changed values
/// under an existing tuple key are `receipt_conflict`.
#[derive(Default)]
pub struct ReceiptDedup {
    seen: HashMap<(String, String, String), String>,
}

impl ReceiptDedup {
    /// `Ok(true)` first sight, `Ok(false)` identical retry.
    pub fn admit(&mut self, receipt: &Receipt) -> AResult<bool> {
        let key = receipt.key();
        let serialized = serde_json::to_string(&receipt.to_json()).unwrap_or_default();
        match self.seen.get(&key) {
            Some(existing) if *existing == serialized => Ok(false),
            Some(_) => Err(AdapterError::named(
                "receipt_conflict",
                "changed values under an existing (adapter_id, session_id, request_id) key",
            )),
            None => {
                self.seen.insert(key, serialized);
                Ok(true)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// Bounded opt-in private JSONL receipt log. A configured cap that cannot
/// fit another <=16 KiB receipt names `usage_log_full`; a write failure is a
/// named error. A new log is created owner-only (0600); an existing log that
/// is readable by other users is refused rather than silently reused. It can
/// never corrupt source or fabricate known totals.
pub struct ReceiptLog {
    file: std::fs::File,
    written: u64,
    cap: u64,
}

impl ReceiptLog {
    pub fn open(path: &Path, cap: u64) -> AResult<Self> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let file = options
            .open(path)
            .map_err(|e| AdapterError::runtime("usage_log_open_failed", e.to_string()))?;
        let metadata = file
            .metadata()
            .map_err(|e| AdapterError::runtime("usage_log_open_failed", e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(AdapterError::named(
                    "usage_log_not_private",
                    "the receipt log must be accessible by its owner only (mode 0600)",
                ));
            }
        }
        Ok(Self {
            file,
            written: metadata.len(),
            cap,
        })
    }

    pub fn append(&mut self, receipt: &Receipt) -> AResult<()> {
        let line = serde_json::to_string(&receipt.to_json()).unwrap_or_default();
        if line.len() > RECEIPT_MAX_BYTES || self.written + line.len() as u64 + 1 > self.cap {
            return Err(AdapterError::runtime(
                "usage_log_full",
                "the receipt log cannot fit another bounded receipt",
            ));
        }
        writeln!(self.file, "{line}")
            .and_then(|()| self.file.flush())
            .map_err(|e| AdapterError::runtime("usage_log_write_failed", e.to_string()))?;
        self.written += line.len() as u64 + 1;
        Ok(())
    }
}

/// Known-totals summary over a JSONL receipt file. Missing usage stays
/// missing; conflicting keys are counted once, never averaged or guessed.
#[derive(Debug, Default, serde::Serialize)]
pub struct Summary {
    pub rows: u64,
    pub receipts_with_usage: u64,
    pub receipts_missing_usage: u64,
    pub duplicate_retries_ignored: u64,
    pub receipt_conflicts: u64,
    pub invalid_rows: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    /// Known cost totals per currency, complete triples only.
    pub cost_microunits: BTreeMap<String, u64>,
    pub outcomes: BTreeMap<String, u64>,
}

fn checked_total(total: &mut u64, value: u64, what: &str) -> AResult<()> {
    *total = total.checked_add(value).ok_or_else(|| {
        AdapterError::runtime(
            "usage_total_overflow",
            format!("the {what} total is not representable as u64; no total is reported"),
        )
    })?;
    Ok(())
}

/// Reads at most 16 MiB and 10,000 rows: excess refuses rather than
/// truncating, and the file is read through a bounded reader so an
/// oversized input is refused without being loaded.
pub fn summarize(path: &Path) -> AResult<Summary> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(SUMMARIZE_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| AdapterError::runtime("usage_input_unreadable", e.to_string()))?;
    if bytes.len() > SUMMARIZE_MAX_BYTES {
        return Err(AdapterError::named(
            "usage_input_too_large",
            format!(
                "usage input exceeds {SUMMARIZE_MAX_BYTES} bytes; refusing rather than truncating"
            ),
        ));
    }
    let mut summary = Summary::default();
    let mut dedup = ReceiptDedup::default();
    for line in bytes.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        summary.rows += 1;
        if summary.rows > SUMMARIZE_MAX_ROWS as u64 {
            return Err(AdapterError::named(
                "usage_input_too_large",
                format!(
                    "usage input exceeds {SUMMARIZE_MAX_ROWS} rows; refusing rather than truncating"
                ),
            ));
        }
        let Ok(receipt) = Receipt::parse(line) else {
            summary.invalid_rows += 1;
            continue;
        };
        match dedup.admit(&receipt) {
            Ok(true) => {}
            Ok(false) => {
                summary.duplicate_retries_ignored += 1;
                continue;
            }
            Err(_) => {
                // The only place a conflict is counted.
                summary.receipt_conflicts += 1;
                continue;
            }
        }
        *summary
            .outcomes
            .entry(receipt.outcome.as_str().to_owned())
            .or_default() += 1;
        if receipt.input_tokens.is_some() || receipt.output_tokens.is_some() {
            summary.receipts_with_usage += 1;
        } else {
            summary.receipts_missing_usage += 1;
        }
        checked_total(
            &mut summary.input_tokens,
            receipt.input_tokens.unwrap_or(0),
            "input token",
        )?;
        checked_total(
            &mut summary.output_tokens,
            receipt.output_tokens.unwrap_or(0),
            "output token",
        )?;
        checked_total(
            &mut summary.cached_input_tokens,
            receipt.cached_input_tokens.unwrap_or(0),
            "cached input token",
        )?;
        if let (Some(cost), Some(currency)) = (receipt.cost_microunits, receipt.currency.as_ref()) {
            checked_total(
                summary.cost_microunits.entry(currency.clone()).or_default(),
                cost,
                "cost",
            )?;
        }
    }
    Ok(summary)
}
