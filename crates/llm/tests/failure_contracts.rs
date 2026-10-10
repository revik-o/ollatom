mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

struct RejectingSink;
impl RunEventSink for RejectingSink {
    fn persist(&self, _event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async { Err(LlmError::EventSink("journal unavailable".into())) })
    }
}

#[derive(Default)]
struct FailFirstFinishedSink {
    failed_sequence: AtomicU64,
    persisted_events: Mutex<Vec<SequencedEvent>>,
}

struct BlockingEventSink;

impl RunEventSink for BlockingEventSink {
    fn persist(&self, _event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(std::future::pending())
    }
}

struct BlockingCompletionSink;

impl RunEventSink for BlockingCompletionSink {
    fn persist(&self, event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if matches!(&event.event, RunEvent::Completed) {
                std::future::pending().await
            } else {
                Ok(())
            }
        })
    }
}

impl RunEventSink for FailFirstFinishedSink {
    fn persist(&self, event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async move {
            if matches!(&event.event, RunEvent::Tool(ToolEvent::Finished { .. }))
                && self
                    .failed_sequence
                    .compare_exchange(0, event.sequence, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Err(LlmError::EventSink("temporary journal failure".into()));
            }

            self.persisted_events.lock().unwrap().push(event);
            Ok(())
        })
    }
}

#[path = "failure_contracts/event_and_history.rs"]
mod event_and_history;
#[path = "failure_contracts/run_lifecycle.rs"]
mod run_lifecycle;
#[path = "failure_contracts/run_limits.rs"]
mod run_limits;
