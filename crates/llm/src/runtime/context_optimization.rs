use super::context_optimization_host::ContextOptimizationProviderHost;
use crate::{
    ContentBlock, ContextOptimizationOutcome, ContextOptimizationRequest, ContextOverflowPolicy,
    ConversationMessage, ConversationRole, LlmError, LlmOptions, LlmProvider, ModelId, ProviderId,
    ProviderRunOutcome, ProviderRunRequest, ReasoningEffort, RunId, RunLimits, StopToken, Usage,
    UserMessage,
};
use std::sync::Arc;

const CONTEXT_OPTIMIZATION_SYSTEM_PROMPT: &str =
    include_str!("prompts/context_optimization_system_prompt.txt");

pub(super) struct ContextOptimizer {
    provider: Arc<dyn LlmProvider>,
    provider_id: ProviderId,
    model_id: ModelId,
    run_id: RunId,
    system_prompt: Option<String>,
    options: LlmOptions,
    stop_token: StopToken,
}

impl ContextOptimizer {
    pub(super) fn new(
        provider: Arc<dyn LlmProvider>,
        provider_id: ProviderId,
        model_id: ModelId,
        run_id: RunId,
        system_prompt: Option<String>,
        options: LlmOptions,
        stop_token: StopToken,
    ) -> Self {
        Self {
            provider,
            provider_id,
            model_id,
            run_id,
            system_prompt,
            options,
            stop_token,
        }
    }

    pub(super) async fn optimize(
        &self,
        context_optimization_request: ContextOptimizationRequest,
    ) -> Result<ContextOptimizationOutcome, LlmError> {
        if !self.options.context.optimization {
            return Ok(unchanged_outcome(context_optimization_request.history));
        }

        if context_optimization_request.input_token_limit == 0 {
            return Err(LlmError::InvalidRequest(
                "context optimization requires a positive input token limit".into(),
            ));
        }

        if self.stop_token.is_stopped() {
            return Err(LlmError::Cancelled);
        }

        let preserved_history = context_optimization_request.history;
        let Some(active_goal_index) = preserved_history
            .iter()
            .rposition(|message| message.role == ConversationRole::User)
        else {
            return Ok(unchanged_outcome(preserved_history));
        };

        let recent_history_start_index =
            crate::context_optimization_selection::recent_history_start_index(
                &preserved_history,
                active_goal_index.saturating_add(1),
            );
        let compacted_history = preserved_history
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != active_goal_index && *index < recent_history_start_index)
            .map(|(_, message)| message.clone())
            .collect::<Vec<_>>();

        if compacted_history.is_empty() {
            return Ok(unchanged_outcome(preserved_history));
        }

        let summary_response = self
            .summarize(&preserved_history[active_goal_index], &compacted_history)
            .await?;
        let summary_text = summary_response.text.trim();

        if summary_text.is_empty() {
            return Ok(ContextOptimizationOutcome {
                model_visible_history: preserved_history.clone(),
                preserved_history,
                usage: summary_response.usage,
                optimized: false,
            });
        }

        let mut model_visible_history =
            Vec::with_capacity(preserved_history.len() - compacted_history.len() + 1);
        model_visible_history.push(ConversationMessage {
            role: ConversationRole::Assistant,
            content: vec![ContentBlock::text(format!(
                "Structured summary of compacted working history:\n{summary_text}"
            ))],
        });
        model_visible_history.push(preserved_history[active_goal_index].clone());
        model_visible_history.extend_from_slice(&preserved_history[recent_history_start_index..]);

        Ok(ContextOptimizationOutcome {
            model_visible_history,
            preserved_history,
            usage: summary_response.usage,
            optimized: true,
        })
    }

    async fn summarize(
        &self,
        active_goal: &ConversationMessage,
        compacted_history: &[ConversationMessage],
    ) -> Result<crate::LlmResponse, LlmError> {
        let serialized_active_goal = serialize_context_fragment(active_goal, "active goal")?;
        let serialized_compacted_history =
            serialize_context_fragment(compacted_history, "compacted history")?;
        let mut optimization_options = self.options.clone();
        optimization_options.context.optimization = false;
        optimization_options.context.input_token_budget = None;
        optimization_options.context.overflow_policy = Some(ContextOverflowPolicy::Error);
        optimization_options.reasoning.effort = ReasoningEffort::Auto;
        optimization_options.reasoning.budget_tokens = None;
        let context_optimization_host = Arc::new(ContextOptimizationProviderHost::new(
            self.stop_token.clone(),
        ));
        let context_optimization_provider_request = ProviderRunRequest {
            run_id: self.run_id,
            provider: self.provider_id.clone(),
            model: self.model_id.clone(),
            system_prompt: Some(optimization_system_prompt(self.system_prompt.as_deref())),
            context: Vec::new(),
            user_message: UserMessage::from(format!(
                "Current goal that must remain exact:\n{serialized_active_goal}\n\nHistory to compact:\n{serialized_compacted_history}"
            )),
            options: optimization_options,
            tools: Vec::new(),
            limits: optimization_run_limits(),
        };
        let context_optimization_provider_outcome = self
            .provider
            .run(
                context_optimization_provider_request,
                context_optimization_host.clone(),
                self.stop_token.clone(),
            )
            .await?;

        super::outcome_validation::validate_identity(
            &context_optimization_provider_outcome,
            &self.provider_id,
            &self.model_id,
        )?;

        match context_optimization_provider_outcome {
            ProviderRunOutcome::Completed(mut response) => {
                context_optimization_host
                    .complete_response(&mut response)
                    .await;
                Ok(response)
            }
            ProviderRunOutcome::Cancelled(_partial_response) => Err(LlmError::Cancelled),
        }
    }
}

fn optimization_system_prompt(original_system_prompt: Option<&str>) -> String {
    match original_system_prompt {
        Some(original_system_prompt) if !original_system_prompt.trim().is_empty() => {
            format!(
                "{CONTEXT_OPTIMIZATION_SYSTEM_PROMPT}\n\nOriginal system constraints to preserve:\n{original_system_prompt}"
            )
        }
        _ => CONTEXT_OPTIMIZATION_SYSTEM_PROMPT.into(),
    }
}

fn serialize_context_fragment<SerializableValue>(
    value: &SerializableValue,
    fragment_name: &str,
) -> Result<String, LlmError>
where
    SerializableValue: serde::Serialize + ?Sized,
{
    serde_json::to_string(value).map_err(|error| {
        LlmError::ProviderProtocol(format!(
            "failed to serialize context optimization {fragment_name}: {error}"
        ))
    })
}

fn unchanged_outcome(history: Vec<ConversationMessage>) -> ContextOptimizationOutcome {
    ContextOptimizationOutcome {
        model_visible_history: history.clone(),
        preserved_history: history,
        usage: Usage::default(),
        optimized: false,
    }
}

fn optimization_run_limits() -> RunLimits {
    RunLimits {
        provider_rounds: 1,
        total_tool_calls: 0,
        tool_calls_per_round: 0,
        child_subagents: 0,
        subagent_depth: 0,
        ..RunLimits::default()
    }
}
