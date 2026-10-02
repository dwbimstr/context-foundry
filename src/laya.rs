//! The existing Laya /v1/systemone protocol. No model weights live in the core.
//! Deterministic fallback routing shares the one keyword policy in
//! `response::strategy_for_query`; the core never duplicates that rule.
use crate::error::{FResult, FoundryError};
use crate::store::{Engine, FEEDBACK};
use anyhow::Result;
use redb::{ReadableDatabase, ReadableTable};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    Auto,
    Search,
    Graph,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Strategy::Auto => "auto",
            Strategy::Search => "search",
            Strategy::Graph => "graph",
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Decision {
    pub strategy: Strategy,
    pub source: &'static str,
    pub answer_confidence: Option<f64>,
    pub fallback_reason: Option<String>,
}

pub fn decide(query: &str, port: Option<u16>, threshold: f64) -> Decision {
    let fallback = crate::response::strategy_for_query(query);
    let mut result = Decision {
        strategy: fallback,
        source: "deterministic",
        answer_confidence: None,
        fallback_reason: None,
    };
    if let Some(port) = port {
        match predict(query, port, threshold) {
            Ok((strategy, confidence)) => {
                result.strategy = strategy;
                result.source = "laya";
                result.answer_confidence = Some(confidence);
            }
            Err(error) => result.fallback_reason = Some(error.to_string()),
        }
    }
    result
}

pub fn predict(query: &str, port: u16, threshold: f64) -> Result<(Strategy, f64)> {
    anyhow::ensure!(
        query.len() <= 4096 && !query.trim().is_empty(),
        "invalid query"
    );
    anyhow::ensure!(
        threshold.is_finite() && (0.0..=1.0).contains(&threshold),
        "invalid confidence threshold"
    );
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .max_redirects(0)
        .proxy(None)
        .build()
        .new_agent();
    let body = json!({"state": query, "questions": {"strategy": {
        "type": "choice", "instructions": "Which retrieval strategy best answers this coding question?",
        "criteria": {"search": "Locate source text or an identifier", "graph": "Find callers, dependencies, or change impact"}
    }}});
    let mut response = agent
        .post(format!("http://127.0.0.1:{port}/v1/systemone"))
        .send_json(body)?;
    let answer: serde_json::Value = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json()?;
    let decision = &answer["answers"]["strategy"];
    anyhow::ensure!(
        decision["type"] == "choice",
        "Laya returned a non-choice decision"
    );
    let confidence = decision["answer_confidence"]
        .as_f64()
        .ok_or_else(|| anyhow::anyhow!("Laya omitted answer_confidence"))?;
    anyhow::ensure!(
        confidence.is_finite() && (threshold..=1.0).contains(&confidence),
        "Laya confidence below policy threshold or invalid"
    );
    let strategy = match decision["choice"].as_str() {
        Some("search") => Strategy::Search,
        Some("graph") => Strategy::Graph,
        _ => anyhow::bail!("Laya returned an unknown strategy"),
    };
    Ok((strategy, confidence))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub task_id: String,
    pub query: String,
    pub correct_strategy: Strategy,
    pub label_source: String,
    pub allow_training: bool,
}

fn invalid(message: &'static str) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
}

impl Engine {
    pub fn record_feedback(&self, feedback: &Feedback) -> FResult<String> {
        if feedback.task_id.trim().is_empty() || feedback.task_id.len() > 256 {
            return Err(invalid("invalid task id"));
        }
        if feedback.query.trim().is_empty() || feedback.query.len() > 4096 {
            return Err(invalid("invalid feedback query"));
        }
        if !["operator", "task_checker"].contains(&feedback.label_source.as_str()) {
            return Err(invalid("label must come from operator or task_checker"));
        }
        let encoded = serde_json::to_string(feedback)?;
        // A correction or consent withdrawal replaces the same task/query example.
        let id =
            crate::digest(serde_json::to_string(&(&feedback.task_id, &feedback.query))?.as_bytes());
        let tx = self.db.begin_write()?;
        {
            tx.open_table(FEEDBACK)?
                .insert(id.as_str(), encoded.as_str())?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// A stable task split prevents different interactions from one task leaking across sets.
    pub fn training_examples(&self) -> FResult<Vec<serde_json::Value>> {
        let tx = self.db.begin_read()?;
        let mut rows = Vec::new();
        for row in tx.open_table(FEEDBACK)?.iter()? {
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
            rows.push(json!({"id": id.value(), "task_id": feedback.task_id, "state": feedback.query,
                "correct_strategy": feedback.correct_strategy, "label_source": feedback.label_source,
                "split": split, "recipe": "retrieval-strategy-v1"}));
        }
        Ok(rows)
    }
}
