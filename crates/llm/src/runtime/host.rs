use super::{context_optimization::ContextOptimizer, interactions::InteractionHub};
use crate::{
    LlmError, RunId, RunLimits, RunPolicy, StopToken, SubagentProfileRegistry, ToolOutput,
    ToolRegistry, events::EventDispatcher, interaction::InteractionCallback,
    subagent::SharedSubagentRunner,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};
use tokio::sync::Mutex;

mod provider;

pub(crate) struct RunHost {
    pub run_id: RunId,
    pub events: Arc<EventDispatcher>,
    pub tools: Arc<ToolRegistry>,
    pub selected_tools: BTreeSet<String>,
    pub policy: RunPolicy,
    pub stop: StopToken,
    pub limits: RunLimits,
    pub overall_deadline: Option<Instant>,
    pub interactions: Arc<InteractionHub>,
    pub interaction_callback: Option<InteractionCallback>,
    pub profiles: Arc<SubagentProfileRegistry>,
    pub subagent_runner: Option<SharedSubagentRunner>,
    pub authorizer: Option<Arc<dyn crate::ToolAuthorizer>>,
    pub parent_context: Vec<crate::ConversationMessage>,
    pub context_optimizer: ContextOptimizer,
    pub(super) state: Mutex<HostState>,
    pub(super) execution: Mutex<()>,
    pub(super) finalized: AtomicBool,
}
#[derive(Default)]
pub(super) struct HostState {
    pub(super) round: u16,
    pub(super) tool_round_executed: bool,
    pub(super) total_calls: u16,
    pub(super) children: u8,
    pub(super) completed_calls: BTreeMap<String, CompletedToolCall>,
    pub(super) child_usage: Vec<crate::Usage>,
    pub(super) tool_records: Vec<crate::ToolExecutionRecord>,
}

pub(super) struct CompletedToolCall {
    pub(super) tool_name: String,
    pub(super) arguments: serde_json::Value,
    pub(super) output: ToolOutput,
    pub(super) finished_event_was_delivered: bool,
}

impl RunHost {
    pub(super) fn finalize(&self) {
        self.finalized.store(true, Ordering::Release);
    }

    pub(super) fn ensure_active(&self) -> Result<(), LlmError> {
        if self.finalized.load(Ordering::Acquire) || self.stop.is_stopped() {
            return Err(LlmError::Cancelled);
        }
        Ok(())
    }

    pub(super) fn is_tool_selected(&self, tool_name: &str) -> bool {
        self.selected_tools.contains(tool_name)
    }
}
