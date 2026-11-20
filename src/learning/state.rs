//! State composition (013 contract § Exact input and identity): the one
//! composer of the `retrieval-route-v1` state, its `graph:` coverage word
//! and the pre-tokenization size guard, with their tests. The learning
//! tables and the `Engine` feedback surface stay in the parent module.
use super::*;

// ---------------------------------------------------------------------------
// State composition (contract § Exact input and identity)
// ---------------------------------------------------------------------------

/// How many lexical locator lines the state carries.
pub const STATE_LOCATORS: usize = 3;

/// The `graph:` coverage word of the current compiler graph, judged against
/// the ONE source revision the ranking was computed at.
///
/// * No producer has a selected snapshot: `graph_unavailable`.
/// * Selected snapshots exist but none is for `revision` (every compiler fact
///   is ineligible): `graph_stale`.
/// * `complete` only when EVERY selected producer is current, its snapshot
///   state is `Complete` and the report for that exact snapshot says its
///   aggregate import coverage is `complete`; otherwise `partial` (an
///   unresolved, unknown, failed or interrupted import, a stale sibling
///   producer, or no matching report - the graph never certifies more than
///   it proved).
fn graph_coverage(
    producers: &[(String, crate::graph::ProducerRow)],
    revision: u64,
) -> FResult<&'static str> {
    let selected: Vec<_> = producers
        .iter()
        .filter_map(|(_, row)| row.selected.as_ref().map(|s| (s, row.latest.as_ref())))
        .collect();
    if selected.is_empty() {
        return Err(fail(
            "graph_unavailable",
            "no compiler graph snapshot is selected for this store",
        ));
    }
    if selected
        .iter()
        .all(|(snapshot, _)| snapshot.tuple.source_revision != revision)
    {
        return Err(fail(
            "graph_stale",
            "every selected compiler graph snapshot predates the current source revision",
        ));
    }
    let complete = selected.iter().all(|(snapshot, latest)| {
        snapshot.tuple.source_revision == revision
            && snapshot.state == crate::graph::SnapshotState::Complete
            && latest.is_some_and(|report| {
                report.snapshot_id == snapshot.tuple.snapshot_id && report.coverage == "complete"
            })
    });
    Ok(if complete { "complete" } else { "partial" })
}

/// The composed state is held to the same 16 KiB pre-tokenization guard as a
/// stored row: refused with a named code, never truncated.
fn guard_state_size(state: String) -> FResult<String> {
    if state.len() > STATE_MAX_BYTES {
        return Err(fail(
            "state_too_large",
            format!(
                "the composed state is {} bytes; the pre-tokenization guard is {STATE_MAX_BYTES}",
                state.len()
            ),
        ));
    }
    Ok(state)
}

impl Engine {
    /// The core-composed `state` of the `retrieval-route-v1` family: the
    /// admitted query text verbatim; LF; `graph: <complete|partial>` from the
    /// current selected compiler graph; then up to [`STATE_LOCATORS`] LF-led
    /// locator lines `<path> <kind> <qualified name>` naming the top delivery
    /// units of 001's two-tier lexical ranking for the same query (a block or
    /// unnamed unit renders its kind alone, as in v2 labels). No handles, no
    /// source text, no trailing LF.
    ///
    /// This is the ONE composer: row creation (`learning compose-state`) and
    /// inference (013 T003) both call it, so a row stores exactly the state
    /// the policy would see. The coverage line is judged at the revision the
    /// ranking was computed at, so both halves describe one snapshot.
    ///
    /// Refusals, all before any model call (the caller then uses
    /// deterministic routing): `graph_unavailable`, `graph_stale` and
    /// `state_too_large` (the composed state is over the 16 KiB
    /// pre-tokenization guard - never truncated), plus the lexical search's
    /// own refusals for a blank or oversized query and an unusable index.
    pub fn compose_route_state(&self, query: &str, control: &Control) -> FResult<String> {
        control.check()?;
        let batch = self.search_candidates(query, None, STATE_LOCATORS, control)?;
        let producers = self.compiler_producers()?;
        let coverage = graph_coverage(&producers, batch.freshness.source_revision)?;
        let mut state = format!("{query}\ngraph: {coverage}");
        for item in batch.items.iter().take(STATE_LOCATORS) {
            let Some(handle) = &item.handle else {
                continue;
            };
            // A locator line is data, never structure: the same sanitizer v2
            // output applies to labels turns every ASCII control character
            // except TAB into `?`, so indexed text (a Markdown heading that
            // decodes to a line break, say) cannot forge a `graph:` line or
            // a locator.
            state.push('\n');
            state.push_str(&crate::response::single_line(&handle.path));
            state.push(' ');
            state.push_str(&crate::response::single_line(&item.label));
        }
        guard_state_size(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{
        ProducerIdentity, ProducerRow, SelectedSnapshot, SnapshotState, SnapshotTuple,
    };
    use crate::scip::ImportReport;

    fn identity() -> ProducerIdentity {
        ProducerIdentity {
            name: "rust-analyzer".into(),
            release_tag: "2026-08-31".into(),
            commit: "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a".into(),
            version_output: "test-producer 1.0".into(),
            binary_sha256: "0".repeat(64),
        }
    }

    /// A producer row selected at `revision`, in `state`, whose latest report
    /// is for the same snapshot with aggregate `coverage` (or no report).
    fn row(revision: u64, state: SnapshotState, report: Option<&str>) -> (String, ProducerRow) {
        let tuple = SnapshotTuple::new(
            identity(),
            "invocation",
            "config",
            "a".repeat(64),
            "b".repeat(64),
            revision,
        );
        let latest = report.map(|coverage| ImportReport {
            snapshot_id: tuple.snapshot_id.clone(),
            coverage: coverage.to_owned(),
            ..ImportReport::default()
        });
        (
            "rust-analyzer".into(),
            ProducerRow {
                selected: Some(SelectedSnapshot { tuple, state }),
                latest,
            },
        )
    }

    fn code(result: FResult<&'static str>) -> String {
        result.map_or_else(|e| e.code().to_owned(), str::to_owned)
    }

    #[test]
    fn no_selected_snapshot_is_graph_unavailable() {
        assert_eq!(code(graph_coverage(&[], 7)), "graph_unavailable");
        // A producer row that never selected a snapshot is not a graph.
        let unselected = ("rust-analyzer".to_owned(), ProducerRow::default());
        assert_eq!(code(graph_coverage(&[unselected], 7)), "graph_unavailable");
    }

    #[test]
    fn every_snapshot_predating_the_revision_is_graph_stale() {
        let rows = [row(6, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "graph_stale");
        // A snapshot for a NEWER revision than the ranking's is also not current.
        let rows = [row(8, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "graph_stale");
    }

    #[test]
    fn complete_requires_a_complete_snapshot_and_a_complete_matching_report() {
        let rows = [row(7, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "complete");
    }

    #[test]
    fn anything_short_of_proven_complete_is_partial() {
        for (name, rows) in [
            (
                "report says partial",
                vec![row(7, SnapshotState::Complete, Some("partial"))],
            ),
            (
                "snapshot partial",
                vec![row(7, SnapshotState::Partial, Some("complete"))],
            ),
            (
                "snapshot still importing",
                vec![row(7, SnapshotState::Importing, Some("complete"))],
            ),
            (
                "no report at all",
                vec![row(7, SnapshotState::Complete, None)],
            ),
        ] {
            assert_eq!(code(graph_coverage(&rows, 7)), "partial", "{name}");
        }
        // A report for a DIFFERENT snapshot proves nothing about the selected one.
        let (name, mut mismatched) = row(7, SnapshotState::Complete, Some("complete"));
        mismatched.latest.as_mut().unwrap().snapshot_id = "f".repeat(64);
        assert_eq!(code(graph_coverage(&[(name, mismatched)], 7)), "partial");
    }

    #[test]
    fn the_worst_producer_decides_and_a_stale_sibling_is_partial() {
        let complete = row(7, SnapshotState::Complete, Some("complete"));
        let partial = row(7, SnapshotState::Complete, Some("partial"));
        assert_eq!(
            code(graph_coverage(&[complete.clone(), partial], 7)),
            "partial"
        );
        // One current producer plus one that predates the revision: the
        // stale producer's facts are ineligible, so the graph is incomplete.
        let stale = row(6, SnapshotState::Complete, Some("complete"));
        assert_eq!(code(graph_coverage(&[complete, stale], 7)), "partial");
    }

    #[test]
    fn locator_text_cannot_forge_structure() {
        assert_eq!(
            crate::response::single_line("alpha\ngraph: complete\r\u{0}\u{1b}tail\tkept"),
            "alpha?graph: complete???tail\tkept"
        );
        // Non-ASCII text, including Unicode line separators, is untouched:
        // the state's only line separator is LF.
        assert_eq!(
            crate::response::single_line("café 日本語 \u{2028}"),
            "café 日本語 \u{2028}"
        );
    }

    #[test]
    fn the_composed_state_is_refused_not_truncated_past_16_kib() {
        let exactly = "x".repeat(STATE_MAX_BYTES);
        assert_eq!(guard_state_size(exactly.clone()).unwrap(), exactly);
        let over = "x".repeat(STATE_MAX_BYTES + 1);
        assert_eq!(
            guard_state_size(over).unwrap_err().code(),
            "state_too_large"
        );
    }
}
