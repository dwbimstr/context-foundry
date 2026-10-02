//! Cooperative cancellation and deadlines shared by CLI, library and MCP.
//!
//! `check()` is called between files, between sweep pages, between index
//! batches and between graph pages. It never interrupts an OS/library call
//! mid-flight; commit boundaries stay atomic.
use crate::error::{FResult, FoundryError};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

#[derive(Debug)]
pub struct Control {
    flag: Arc<AtomicBool>,
    deadline: Option<Instant>,
    /// 0 none, 1 cancelled, 2 deadline exceeded — set by the failing check.
    last_error: AtomicU64,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            deadline: None,
            last_error: AtomicU64::new(0),
        }
    }
}

impl Control {
    pub fn unbounded() -> Self {
        Self::default()
    }

    /// Already cancelled; the next `check()` fails.
    pub fn cancelled() -> Self {
        let control = Self::default();
        control.flag.store(true, Ordering::SeqCst);
        control
    }

    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            ..Self::default()
        }
    }

    /// Shared cancel flag for signal handlers and MCP session cancellation.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst) || self.deadline.is_some_and(|d| Instant::now() >= d)
    }

    /// One cooperative checkpoint.
    pub fn check(&self) -> FResult<()> {
        if self.flag.load(Ordering::SeqCst) {
            self.last_error.store(1, Ordering::SeqCst);
            return Err(FoundryError::Cancelled(None));
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.last_error.store(2, Ordering::SeqCst);
            return Err(FoundryError::DeadlineExceeded(None));
        }
        Ok(())
    }

    /// Which cooperative check failed last: `cancelled`, `deadline_exceeded`
    /// or `None` when no checkpoint has failed.
    pub fn last_check_error(&self) -> Option<&'static str> {
        match self.last_error.load(Ordering::SeqCst) {
            1 => Some("cancelled"),
            2 => Some("deadline_exceeded"),
            _ => None,
        }
    }
}
