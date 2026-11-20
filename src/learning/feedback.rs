//! Feedback rows: the strict v4 row (013 contract § Feedback and
//! permission) with its identities, and the legacy 001 row with the two
//! `Engine` calls that record and export it. The v4 store surface and the
//! group history stay in the parent module.
use super::*;

// ---------------------------------------------------------------------------
// Strict row parsing
// ---------------------------------------------------------------------------

/// The v4 feedback row (contract § Feedback and permission). Stored under
/// its example ID; a correction replaces label/consent transactionally and
/// the same input is idempotent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRowV4 {
    pub task_id: String,
    pub task_group_id: String,
    pub family: String,
    pub state: String,
    pub option_ids: Vec<String>,
    pub correct_option_id: String,
    pub label_source: String,
    pub label_evidence: String,
    pub rights_ref: String,
    pub allow_training: bool,
}

impl FeedbackRowV4 {
    /// Parse one row strictly: bounded, no duplicate JSON keys, no unknown
    /// or null fields, exact family/options/label-source, byte limits.
    /// Rejects BEFORE any mutation.
    pub fn parse(raw: &str) -> FResult<Self> {
        if raw.len() > WHOLE_ROW_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!(
                    "row is {} bytes; the limit is {WHOLE_ROW_MAX_BYTES}",
                    raw.len()
                ),
            ));
        }
        let row: FeedbackRowV4 = strict_json(raw.as_bytes(), "row_invalid", "v4 feedback row")?;
        row.validate()?;
        Ok(row)
    }

    pub(crate) fn validate(&self) -> FResult<()> {
        let id = |name: &str, value: &str| -> FResult<()> {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(fail(
                    "row_invalid",
                    format!("{name} must be nonblank and at most 256 UTF-8 bytes"),
                ));
            }
            Ok(())
        };
        id("task_id", &self.task_id)?;
        id("task_group_id", &self.task_group_id)?;
        if self.family != FAMILY {
            return Err(fail(
                "row_invalid",
                format!(
                    "unknown family {:?}; only {FAMILY} is supported",
                    self.family
                ),
            ));
        }
        if self.state.is_empty() || self.state.len() > STATE_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!(
                    "state must be nonempty and at most {STATE_MAX_BYTES} bytes before tokenization"
                ),
            ));
        }
        // Exactly the two stable options: neither omitted, nor duplicated.
        if self.option_ids.len() != 2 || self.option_ids[0] == self.option_ids[1] {
            return Err(fail(
                "row_invalid",
                "option_ids must be the two distinct stable option ids in order",
            ));
        }
        for option in &self.option_ids {
            if !decision_model::OPTIONS.iter().any(|def| def.id == *option) {
                return Err(fail(
                    "row_invalid",
                    format!("unknown option id {option:?} for family {FAMILY}"),
                ));
            }
        }
        if !self.option_ids.contains(&self.correct_option_id) {
            return Err(fail(
                "row_invalid",
                "correct_option_id must be one of option_ids (labels map by stable id)",
            ));
        }
        if !["operator", "task_checker"].contains(&self.label_source.as_str()) {
            return Err(fail(
                "row_invalid",
                "label_source must be operator or task_checker",
            ));
        }
        for (name, value) in [
            ("label_evidence", &self.label_evidence),
            ("rights_ref", &self.rights_ref),
        ] {
            if value.trim().is_empty() || value.len() > 1024 {
                return Err(fail(
                    "row_invalid",
                    format!("{name} must be nonblank and at most 1024 UTF-8 bytes"),
                ));
            }
        }
        let bytes = crate::json_len(self).map_err(FoundryError::from)?;
        if bytes > WHOLE_ROW_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!("whole row is {bytes} bytes; the limit is {WHOLE_ROW_MAX_BYTES}"),
            ));
        }
        Ok(())
    }

    /// The ordered option pair for rendering.
    pub fn ordered_options(&self) -> [&str; 2] {
        [self.option_ids[0].as_str(), self.option_ids[1].as_str()]
    }

    /// `input_sha256` over the ORIGINAL state and the ordered option IDs.
    pub fn input_sha256(&self) -> String {
        decision_model::input_sha256(&self.state, self.ordered_options())
    }

    /// Example ID: `SHA256(compact JSON [task_id,input_sha256])`.
    pub fn example_id(&self) -> String {
        compact_digest(&serde_json::json!([self.task_id, self.input_sha256()]))
    }

    /// Permission identity: `SHA256(compact JSON [allow_training,
    /// rights_ref])`. A changed rights assertion is a changed permission even
    /// when consent, input and label stay the same.
    pub fn permission_sha256(&self) -> String {
        compact_digest(&serde_json::json!([self.allow_training, self.rights_ref]))
    }

    /// Duplicate fingerprint: SHA256 of the family, whitespace-normalized
    /// state and options sorted by stable ID. CRLF becomes LF, runs of ASCII
    /// whitespace collapse to one space and the ends trim; no case folding.
    pub fn fingerprint(&self) -> String {
        let normalized = self.state.replace("\r\n", "\n");
        let mut collapsed = String::with_capacity(normalized.len());
        let mut pending_space = false;
        for ch in normalized.chars() {
            if ch.is_ascii_whitespace() {
                pending_space = true;
            } else {
                if pending_space && !collapsed.is_empty() {
                    collapsed.push(' ');
                }
                pending_space = false;
                collapsed.push(ch);
            }
        }
        let mut options: Vec<&str> = self.option_ids.iter().map(String::as_str).collect();
        options.sort_unstable();
        compact_digest(&serde_json::json!([FAMILY, collapsed, options]))
    }
}

// ---------------------------------------------------------------------------
// Legacy feedback (001)
// ---------------------------------------------------------------------------

/// The legacy v1 feedback row (`foundry feedback` with no subcommand): a
/// caller-labeled task/query pair and explicit permission. It lacks the
/// exact inputs and rights v4 requires, so it stays exportable
/// (`export-training`) and is NEVER eligible for training.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub task_id: String,
    pub query: String,
    pub correct_strategy: crate::Strategy,
    pub label_source: String,
    pub allow_training: bool,
}

fn feedback_invalid(message: &'static str) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
}

impl Engine {
    pub fn record_feedback(&self, feedback: &Feedback) -> FResult<String> {
        if feedback.task_id.trim().is_empty() || feedback.task_id.len() > 256 {
            return Err(feedback_invalid("invalid task id"));
        }
        if feedback.query.trim().is_empty() || feedback.query.len() > 4096 {
            return Err(feedback_invalid("invalid feedback query"));
        }
        if !["operator", "task_checker"].contains(&feedback.label_source.as_str()) {
            return Err(feedback_invalid(
                "label must come from operator or task_checker",
            ));
        }
        let encoded = serde_json::to_string(feedback)?;
        // A correction or consent withdrawal replaces the same task/query example.
        let id =
            crate::digest(serde_json::to_string(&(&feedback.task_id, &feedback.query))?.as_bytes());
        let tx = self.db.begin_write()?;
        {
            tx.open_table(crate::store::FEEDBACK)?
                .insert(id.as_str(), encoded.as_str())?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// A stable task split prevents different interactions from one task leaking across sets.
    pub fn training_examples(&self) -> FResult<Vec<serde_json::Value>> {
        let tx = self.db.begin_read()?;
        let mut rows = Vec::new();
        for row in tx.open_table(crate::store::FEEDBACK)?.iter()? {
            let (id, encoded) = row?;
            let feedback: Feedback = serde_json::from_str(encoded.value())
                .map_err(|e| FoundryError::CorruptStore(format!("feedback record: {e}")))?;
            if !feedback.allow_training {
                continue;
            }
            let hash = crate::digest(feedback.task_id.as_bytes());
            let bucket = u8::from_str_radix(&hash[..2], 16)?;
            let split = if bucket == 0 {
                "evaluation"
            } else if bucket == 1 {
                "calibration"
            } else {
                "train"
            };
            rows.push(serde_json::json!({"id": id.value(), "task_id": feedback.task_id, "state": feedback.query,
                "correct_strategy": feedback.correct_strategy, "label_source": feedback.label_source,
                "split": split, "recipe": "retrieval-strategy-v1"}));
        }
        Ok(rows)
    }
}
