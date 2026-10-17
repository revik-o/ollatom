use super::ModelRunState;
use crate::{
    content::NativeTool,
    content_history::native_messages_from_history,
    provider::OllamaProvider,
    request::{
        build_options, context_input_limit, estimate_serialized_tokens, model_is_gemma_four, validate_model_features,
    },
    run_request::{ConversationHistory, optimize_context_if_needed},
};
use llm::{
    ContextOverflowPolicy, LlmError, LocalOptionPhase, OptionHandlingMode, ProviderRunHost, ProviderRunRequest,
    RunEvent, StopToken,
};
use std::collections::BTreeSet;
use std::sync::Arc;

pub(crate) async fn prepare_model_run(
    provider: &OllamaProvider,
    request: &ProviderRunRequest,
    host: &Arc<dyn ProviderRunHost>,
    stop: &StopToken,
) -> Result<ModelRunState, LlmError> {
    let model_details = provider
        .fetch_model_details_with_stop(
            &request.model,
            false,
            stop,
            request
                .options
                .transport
                .connect_timeout_ms
                .map(std::time::Duration::from_millis),
        )
        .await?;

    validate_model_features(&model_details, request)?;

    let (native_options, native_reasoning) = build_options(request)?;

    emit_omitted_option_warnings(request, host).await?;

    let model_is_gemma_four = model_is_gemma_four(&request.model);
    let input_token_limit = context_input_limit(request, &model_details);
    let mut conversation_history = ConversationHistory::from_request(request);
    let initial_history_tokens = estimate_serialized_tokens(&(
        request.system_prompt.as_deref(),
        &conversation_history.model_visible,
        &request.tools,
    ))?;

    optimize_context_if_needed(
        request,
        host,
        &mut conversation_history,
        input_token_limit,
        initial_history_tokens,
        0,
    )
    .await?;

    let native_messages = native_messages_from_history(
        request.system_prompt.as_deref(),
        &conversation_history.model_visible,
        model_is_gemma_four,
    )
    .await?;

    Ok(ModelRunState {
        native_options,
        native_reasoning,
        native_tools: build_native_tools(request),
        native_messages,
        conversation_history,
        input_token_limit,
        model_is_gemma_four,
        observed_tool_call_identifiers: BTreeSet::new(),
    })
}

async fn emit_omitted_option_warnings(
    request: &ProviderRunRequest,
    host: &Arc<dyn ProviderRunHost>,
) -> Result<(), LlmError> {
    if request.options.reasoning.budget_tokens.is_some() {
        if request.options.handling == OptionHandlingMode::Strict {
            return Err(LlmError::UnsupportedOption("reasoning.budget_tokens".into()));
        }

        host.emit(RunEvent::Warning(
            "reasoning token budgets are not supported by Ollama and were omitted".into(),
        ))
        .await?;
    }

    if request.options.context.input_token_budget.is_some() && request.options.handling == OptionHandlingMode::BestEffort
    {
        host.emit(RunEvent::Warning(
            "input token budgets are not supported by Ollama and were omitted".into(),
        ))
        .await?;
    }

    if matches!(
        request.options.context.overflow_policy,
        Some(ContextOverflowPolicy::Summarize)
    ) && !request.options.context.optimization
        && request.options.handling == OptionHandlingMode::BestEffort
    {
        host.emit(RunEvent::Warning(
            "context summarization is not supported by Ollama and was omitted".into(),
        ))
        .await?;
    }

    if request
        .options
        .local
        .required_phase
        .is_some_and(|phase| phase != LocalOptionPhase::PerRequest)
        && request.options.handling == OptionHandlingMode::BestEffort
    {
        host.emit(RunEvent::Warning(
            "required option phase is not supported by Ollama and was omitted".into(),
        ))
        .await?;
    }

    Ok(())
}

fn build_native_tools(request: &ProviderRunRequest) -> Vec<NativeTool> {
    request
        .tools
        .iter()
        .map(|tool| NativeTool {
            r#type: "function".into(),
            function: crate::content::NativeToolFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            },
        })
        .collect()
}
