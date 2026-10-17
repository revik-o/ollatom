use super::{DispatchState, EventDispatcher, RunEventSink, SequencedEvent};
use crate::LlmError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::MutexGuard;

impl EventDispatcher {
    pub(super) async fn lock_for_event_delivery(
        &self,
        event_delivery_deadline: Option<Instant>,
    ) -> Result<MutexGuard<'_, DispatchState>, LlmError> {
        let timeout = self.remaining_event_delivery_time(event_delivery_deadline);

        if timeout.is_zero() {
            return Err(self.event_delivery_timeout_error("waiting for dispatcher state"));
        }

        tokio::time::timeout(timeout, self.state.lock())
            .await
            .map_err(|_| self.event_delivery_timeout_error("waiting for dispatcher state"))
    }

    pub(super) async fn persist_before_deadline(
        &self,
        event_sink: &Arc<dyn RunEventSink>,
        event: SequencedEvent,
        event_delivery_deadline: Option<Instant>,
    ) -> Result<(), LlmError> {
        let timeout = self.remaining_event_delivery_time(event_delivery_deadline);

        if timeout.is_zero() {
            return Err(self.event_delivery_timeout_error("persisting a run event"));
        }

        match tokio::time::timeout(timeout, event_sink.persist(event)).await {
            Ok(result) => result,
            Err(_) => Err(self.event_delivery_timeout_error("persisting a run event")),
        }
    }

    fn remaining_event_delivery_time(&self, event_delivery_deadline: Option<Instant>) -> Duration {
        let event_delivery_remaining = event_delivery_deadline.map_or(self.event_delivery_timeout, |deadline| {
            deadline.saturating_duration_since(Instant::now())
        });

        self.overall_deadline.map_or(event_delivery_remaining, |deadline| {
            event_delivery_remaining.min(deadline.saturating_duration_since(Instant::now()))
        })
    }

    fn event_delivery_timeout_error(&self, operation: &str) -> LlmError {
        self.overall_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
            .then_some(LlmError::Timeout(format!(
                "overall run deadline expired while {operation}"
            )))
            .unwrap_or(LlmError::EventSink(format!(
                "event delivery exceeded {:?} while {operation}",
                self.event_delivery_timeout
            )))
    }
}
