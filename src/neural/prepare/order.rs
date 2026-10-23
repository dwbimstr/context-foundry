//! Source scheduling: the cursor of one source wave and the order that
//! alternates explicit priority paths with the background walk. The CLI loop
//! and the shared steps stay in the parent module.
use super::*;

#[derive(Clone, Default)]
pub(crate) struct SourceCursor {
    pub exact: bool,
    pub scope: Option<String>,
    pub after: Option<String>,
    /// Scalar progress in an unfinished wave; `after` retains its upper bound.
    /// Bodies/plans live only in the run's single byte-capped paused cache.
    pub resume: Option<WalkPosition>,
}
impl SourceCursor {
    /// The exclusive lower bound of the next wave: a paused wave's scalar
    /// progress, else `after`. Read it before partitioning moves `after`.
    pub(crate) fn wave_start(&self) -> Option<String> {
        match &self.resume {
            Some(position) => position.after.clone(),
            None => self.after.clone(),
        }
    }

    /// Consumes `resume`. Counts and opens the wave from `before` through
    /// `after`, restoring a paused position; `None` when the cursor did not
    /// move.
    pub(crate) fn open_walk(
        &mut self,
        before: Option<String>,
        retained: Vec<PreparedCards>,
        overview_first: bool,
        paused: &mut PausedSources,
        report: &mut PrepareReport,
    ) -> Option<Walk> {
        let resumed = self.resume.take();
        if self.after == before {
            return None;
        }
        report.bootstrap.waves += 1;
        report.bootstrap.priority_waves += u64::from(self.scope.is_some());
        let mut walk =
            Walk::wave(self.scope.clone(), before, self.after.clone()).with_retained(retained);
        if let Some(position) = resumed {
            walk = walk.restore(position, paused);
        }
        walk.exact = self.exact;
        walk.overview_first = overview_first;
        Some(walk)
    }
}

/// At most sixteen priority cursors and one ordinary cursor per pass. Hierarchy
/// keeps one additional copy for refinement, with no retained bodies/vectors.
/// Alternate priority/background waves so neither starves.
#[derive(Clone)]
pub(crate) struct SourceOrder {
    priority: VecDeque<SourceCursor>,
    background: Option<SourceCursor>,
    priority_turn: bool,
    overview_first: bool,
    refinement: Option<Box<SourceOrder>>,
}
impl SourceOrder {
    pub(crate) fn changed(paths: impl Iterator<Item = String>) -> Self {
        Self {
            priority: paths
                .map(|path| SourceCursor {
                    scope: Some(path),
                    after: None,
                    exact: true,
                    resume: None,
                })
                .collect(),
            background: None,
            priority_turn: true,
            overview_first: false,
            refinement: None,
        }
    }
    pub(crate) fn prioritize(&mut self, paths: &[String]) -> FResult<()> {
        self.priority = Self::new(paths)?.priority;
        if let Some(refinement) = &mut self.refinement {
            refinement.prioritize(paths)?;
        }
        Ok(())
    }
    /// Hierarchy partitions already begin with a whole-file representation.
    /// Visit that input in every selected source before draining body inputs.
    /// Only cursors are copied; accepted work remains in the ordinary cache.
    pub(crate) fn for_representation(mut self, representation: partition::Representation) -> Self {
        if matches!(representation, partition::Representation::Hierarchy { .. }) {
            self.refinement = Some(Box::new(self.clone()));
            self.overview_first = true;
        }
        self
    }
    pub(crate) fn overview_first(&self) -> bool {
        self.overview_first
    }
    pub(crate) fn new(paths: &[String]) -> FResult<Self> {
        validate_priority_paths(paths)?;
        let mut unique = Vec::new();
        for path in paths {
            if !unique.contains(path) {
                unique.push(path.clone());
            }
        }
        Ok(Self {
            priority: unique
                .into_iter()
                .map(|scope| SourceCursor {
                    scope: Some(scope),
                    after: None,
                    exact: false,
                    resume: None,
                })
                .collect(),
            background: Some(SourceCursor::default()),
            priority_turn: true,
            overview_first: false,
            refinement: None,
        })
    }
    pub(crate) fn next(&mut self) -> Option<SourceCursor> {
        if !self.waiting()
            && let Some(refinement) = self.refinement.take()
        {
            *self = *refinement;
        }
        if (self.priority_turn || self.background.is_none()) && !self.priority.is_empty() {
            self.priority_turn = false;
            self.priority.pop_front()
        } else {
            self.priority_turn = true;
            self.background.take()
        }
    }
    pub(crate) fn waiting(&self) -> bool {
        !self.priority.is_empty() || self.background.is_some()
    }
    /// Inputs the active wave may still select: `requested`, capped at
    /// [`WAVE_INPUTS`] per wave while other cursors wait.
    pub(crate) fn selection_room(&self, requested: usize, used: usize) -> usize {
        if self.waiting() {
            requested.min(WAVE_INPUTS - used)
        } else {
            requested
        }
    }
    pub(crate) fn advance(&mut self, cursor: SourceCursor, done: bool) {
        if done {
            return;
        }
        if cursor.scope.is_some() {
            self.priority.push_back(cursor);
        } else {
            self.background = Some(cursor);
        }
    }
}
