use crate::{
    ConversationMessage, EventCallbacks, LlmOptions, ModelId, ProviderId, RunLimits, RunPolicy,
    UserMessage, interaction::InteractionCallback,
};
use std::collections::BTreeSet;

pub(crate) struct RequestData {
    pub runtime: Option<crate::LlmRuntime>,
    pub provider: Option<ProviderId>,
    pub model: Option<ModelId>,
    pub options: LlmOptions,
    pub explicit_effort: Option<crate::ReasoningEffort>,
    pub system_prompt: Option<String>,
    pub context: Vec<ConversationMessage>,
    pub user_message: Option<UserMessage>,
    pub selected_tools: BTreeSet<String>,
    pub policy: RunPolicy,
    pub callbacks: EventCallbacks,
    pub interaction_callback: Option<InteractionCallback>,
    pub limits: RunLimits,
    pub validation_error: Option<crate::LlmError>,
}
