pub use crate::event_queue::RunEventStream;
use crate::{
    BoxFuture, FinishReason, InteractionRequest, LlmError, RunId, ToolCall, ToolOutput, Usage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod dispatcher;
mod streamed_output;

pub(crate) use dispatcher::EventDispatcher;
pub(crate) use streamed_output::StreamedOutput;

const CALLBACK_QUEUE_CAPACITY: usize = 1_024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ToolEvent {
    Planned { call: ToolCall },
    ApprovalRequested { call_id: String },
    Started { call: ToolCall },
    Cancelled { call_id: String, reason: String },
    Finished { output: ToolOutput },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunEvent {
    ReasoningSummaryDelta(String),
    ModelTraceDelta(String),
    ResponseDelta(String),
    Tool(ToolEvent),
    Usage(Usage),
    Warning(String),
    InteractionRequested(InteractionRequest),
    ProviderRoundStarted {
        round: u16,
    },
    ProviderRoundFinished {
        round: u16,
        finish_reason: FinishReason,
        tool_calls: Vec<ToolCall>,
        usage: Usage,
    },
    ContextOptimized {
        original_message_count: usize,
        optimized_message_count: usize,
        observed_input_tokens: u64,
        input_token_limit: u64,
    },
    Completed,
    Cancelled,
    Failed(String),
}

impl RunEvent {
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::Tool(_)
                | Self::InteractionRequested(_)
                | Self::ProviderRoundFinished { .. }
                | Self::ContextOptimized { .. }
                | Self::Completed
                | Self::Cancelled
                | Self::Failed(_)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed(_))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SequencedEvent {
    pub run_id: RunId,
    pub sequence: u64,
    pub event: RunEvent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum RunStreamItem {
    Event(SequencedEvent),
    CoalescedDelta(CoalescedRunDelta),
    Lagged(RunStreamLag),
    DeliveryFailed(RunEventDeliveryFailure),
}

impl RunStreamItem {
    pub fn event(&self) -> Option<&RunEvent> {
        match self {
            Self::Event(event) => Some(&event.event),
            Self::CoalescedDelta(delta) => Some(&delta.event),
            Self::Lagged(_) | Self::DeliveryFailed(_) => None,
        }
    }

    pub fn into_event(self) -> Option<RunEvent> {
        match self {
            Self::Event(event) => Some(event.event),
            Self::CoalescedDelta(delta) => Some(delta.event),
            Self::Lagged(_) | Self::DeliveryFailed(_) => None,
        }
    }

    pub fn sequence_range(&self) -> Option<(u64, u64)> {
        match self {
            Self::Event(event) => Some((event.sequence, event.sequence)),
            Self::CoalescedDelta(delta) => Some((delta.first_sequence, delta.last_sequence)),
            Self::Lagged(_) | Self::DeliveryFailed(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoalescedRunDelta {
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub event: RunEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStreamLag {
    pub first_missing_sequence: u64,
    pub last_missing_sequence: u64,
    pub missing_event_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEventDeliveryFailure {
    pub sequence: u64,
    pub message: String,
}

pub trait RunEventSink: Send + Sync {
    fn persist(&self, event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>>;
}

pub type EventCallback = Arc<dyn Fn(SequencedEvent) -> BoxFuture<'static, ()> + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventCallbacks {
    callbacks: Vec<EventCallback>,
}

impl EventCallbacks {
    pub fn push(&mut self, callback: EventCallback) {
        self.callbacks.push(callback);
    }
}
