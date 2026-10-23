//! The `Owner` seams through which the owner's background subsystems
//! (semantic preparation, knowledge maintenance, source watch) take the one
//! engine slot and model admission. `Shared`, the slot and `run_op` stay in
//! the parent module.
use super::*;

impl Shared {
    /// 009 T003: the engine slot for one background step, marked
    /// [`SLOT_DRIVER`] and taken only while no foreground operation is in
    /// flight; `None` when either refuses. An operation counted before the
    /// slot was taken goes first; one counted from here on waits for the step.
    fn try_driver_slot(&self) -> FResult<Option<SlotGuard<'_>>> {
        if self.in_flight_engine.load(Ordering::SeqCst) > 0 {
            return Ok(None);
        }
        let slot = match self.engines.try_lock() {
            Ok(engines) => SlotGuard::mark(engines, &self.slot_holder, SLOT_DRIVER),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(FoundryError::Internal(anyhow::anyhow!(
                    "engine state was poisoned by an earlier panic"
                )));
            }
        };
        if self.in_flight_engine.load(Ordering::SeqCst) > 0 {
            return Ok(None);
        }
        Ok(Some(slot))
    }
}

/// 009 T003: the preparation driver's access to the owner. A store step runs
/// under the one engine slot, taken only while no foreground operation is in
/// flight, so foreground operations go first; one arriving during the step
/// waits for it ([`take_engine_slot`]).
#[cfg(feature = "semantic")]
impl crate::neural::driver::Owner for Shared {
    fn try_model(&self) -> Option<crate::neural::work::ModelPermit> {
        if self
            .model_resources
            .as_ref()
            .is_some_and(|resources| resources.background().is_err())
        {
            return None;
        }
        crate::neural::work::ModelPermit::try_acquire(&self.background_model)
    }
    fn try_primary(&self, step: &mut dyn FnMut(&Engine)) -> FResult<bool> {
        if self
            .watch
            .as_ref()
            .is_some_and(|watch| watch.source_pending())
        {
            if self.source_steps.load(Ordering::SeqCst) < 8 {
                return Ok(false);
            }
            *self
                .semantic_turn
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
        }
        let Some(engines) = self.try_driver_slot()? else {
            return Ok(false);
        };
        let Some(engine) = engines.first().and_then(Option::as_ref) else {
            return Err(FoundryError::Internal(anyhow::anyhow!(
                "the primary engine is not open"
            )));
        };
        self.source_steps.store(0, Ordering::SeqCst);
        *self
            .semantic_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        step(engine);
        Ok(true)
    }

    fn closing(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

impl crate::knowledge::maintenance::Owner for Shared {
    fn try_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
        let Some(mut engines) = self.try_driver_slot()? else {
            return Ok(false);
        };
        step(engines[0].as_mut().expect("primary engine is open"));
        Ok(true)
    }
    fn try_model(&self) -> FResult<Option<crate::knowledge::maintenance::Permit>> {
        if self.in_flight_engine.load(Ordering::SeqCst) > 0
            || self
                .watch
                .as_ref()
                .is_some_and(|watch| watch.source_pending())
        {
            return Ok(None);
        }
        let Some(model) = crate::neural::work::ModelPermit::try_acquire(&self.background_model)
        else {
            return Ok(None);
        };
        #[cfg(feature = "semantic")]
        let runtime = if let Some(Ok(runtime)) = &self.semantic {
            match runtime.admit_knowledge() {
                Ok(permit) => Some(permit),
                Err(crate::neural::provider::ProviderError::Busy) => return Ok(None),
                Err(error) => {
                    return Err(crate::knowledge::fail(
                        "knowledge_admission",
                        &error.to_string(),
                    ));
                }
            }
        } else {
            None
        };
        Ok(Some(crate::knowledge::maintenance::Permit {
            _model: model,
            #[cfg(feature = "semantic")]
            _runtime: runtime,
        }))
    }
    fn closing(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}

/// Source maintenance uses the SAME slot and foreground admission as semantic
/// preparation. Each callback is one file, walk entry, deletion or index page.
impl crate::watch::Owner for Shared {
    fn try_watch_step(&self, step: &mut dyn FnMut(&mut Engine)) -> FResult<bool> {
        if self.shutdown.is_cancelled() {
            return Ok(false);
        }
        #[cfg(feature = "semantic")]
        if self
            .semantic_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < Duration::from_millis(100))
        {
            return Ok(false);
        }
        let Some(mut engines) = self.try_driver_slot()? else {
            return Ok(false);
        };
        if self.shutdown.is_cancelled() {
            return Ok(false);
        }
        let engine = engines[0].as_mut().expect("primary engine is open");
        #[cfg(feature = "semantic")]
        let _ = self
            .source_steps
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |steps| {
                Some((steps + 1).min(8))
            });
        step(engine);
        Ok(true)
    }
    fn closing(&self) -> bool {
        self.shutdown.is_cancelled()
    }
}
