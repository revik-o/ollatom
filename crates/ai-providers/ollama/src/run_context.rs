use super::{
    content::{NativeMessage, NativeTool},
    run_request::ConversationHistory,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

mod initialization;
mod tool_round;

pub(super) use initialization::prepare_model_run;
pub(super) use tool_round::{continue_after_tool_round, emit_provider_round_finished};

pub(super) struct ModelRunState {
    pub(super) native_options: Map<String, Value>,
    pub(super) native_reasoning: Option<Value>,
    pub(super) native_tools: Vec<NativeTool>,
    pub(super) native_messages: Vec<NativeMessage>,
    pub(super) conversation_history: ConversationHistory,
    pub(super) input_token_limit: Option<u64>,
    pub(super) model_is_gemma_four: bool,
    pub(super) observed_tool_call_identifiers: BTreeSet<String>,
}
