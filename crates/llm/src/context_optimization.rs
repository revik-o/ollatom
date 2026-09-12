use crate::{ConversationMessage, Usage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextOptimizationRequest {
    pub history: Vec<ConversationMessage>,
    pub observed_input_tokens: u64,
    pub input_token_limit: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextOptimizationOutcome {
    pub model_visible_history: Vec<ConversationMessage>,
    pub preserved_history: Vec<ConversationMessage>,
    pub usage: Usage,
    pub optimized: bool,
}
