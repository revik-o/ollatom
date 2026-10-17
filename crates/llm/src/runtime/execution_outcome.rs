use crate::{
    FailedRun, LlmError, LlmRunOutcome, PartialResponse, ProviderRunOutcome, ToolExecutionRecord, Usage,
    events::EventDispatcher,
};

pub(super) struct HostResults {
    child_usage: Vec<Usage>,
    tool_executions: Vec<ToolExecutionRecord>,
}

impl HostResults {
    pub(super) fn new(child_usage: Vec<Usage>, tool_executions: Vec<ToolExecutionRecord>) -> Self {
        Self {
            child_usage,
            tool_executions,
        }
    }
}

pub(super) fn attach_host_results(
    mut provider_outcome: ProviderRunOutcome,
    host_results: HostResults,
) -> ProviderRunOutcome {
    let (usage, tool_executions) = match &mut provider_outcome {
        ProviderRunOutcome::Completed(response) => (&mut response.usage, &mut response.tool_executions),
        ProviderRunOutcome::Cancelled(partial_response) => {
            (&mut partial_response.usage, &mut partial_response.tool_executions)
        }
    };

    usage.child_usage.extend(host_results.child_usage);
    *tool_executions = host_results.tool_executions;

    provider_outcome
}

pub(super) async fn cancelled_outcome(
    provider_result: Result<ProviderRunOutcome, LlmError>,
    provider_id: crate::ProviderId,
    model_id: crate::ModelId,
    event_dispatcher: &EventDispatcher,
    host_results: HostResults,
) -> LlmRunOutcome {
    match provider_result {
        Ok(provider_outcome) => match attach_host_results(provider_outcome, host_results) {
            ProviderRunOutcome::Completed(response) => LlmRunOutcome::Cancelled(response.into()),

            ProviderRunOutcome::Cancelled(partial_response) => LlmRunOutcome::Cancelled(partial_response),
        },
        Err(_) => {
            let streamed_output = event_dispatcher.streamed_output().await;
            let assistant_rounds = streamed_output.assistant_round_snapshot();
            let mut usage = streamed_output.usage;

            usage.child_usage.extend(host_results.child_usage);

            LlmRunOutcome::Cancelled(PartialResponse {
                provider: provider_id,
                model: model_id,
                assistant_rounds,
                tool_executions: host_results.tool_executions,
                usage,
            })
        }
    }
}

pub(super) async fn failed_outcome(
    error: LlmError,
    provider_id: crate::ProviderId,
    model_id: crate::ModelId,
    event_dispatcher: &EventDispatcher,
    host_results: HostResults,
    provider_outcome: Option<ProviderRunOutcome>,
) -> LlmRunOutcome {
    let streamed_output = event_dispatcher.streamed_output().await;
    let mut partial_response = match provider_outcome {
        Some(ProviderRunOutcome::Completed(response)) => PartialResponse::from(response),
        Some(ProviderRunOutcome::Cancelled(partial_response)) => partial_response,
        None => PartialResponse {
            provider: provider_id.clone(),
            model: model_id.clone(),
            assistant_rounds: Vec::new(),
            tool_executions: Vec::new(),
            usage: Usage::default(),
        },
    };

    if partial_response.assistant_rounds.is_empty() {
        partial_response.assistant_rounds = streamed_output.assistant_round_snapshot();
    }

    if partial_response.usage == Usage::default() {
        partial_response.usage = streamed_output.usage;
    }

    partial_response.provider = provider_id;
    partial_response.model = model_id;
    partial_response.tool_executions = host_results.tool_executions;

    let mut usage = partial_response.usage;

    usage.child_usage.extend(host_results.child_usage);
    partial_response.usage = usage;

    LlmRunOutcome::Failed(FailedRun {
        error,
        partial_response,
        terminal_event_delivered: false,
    })
}

pub(super) fn empty_cancelled_outcome(provider_id: crate::ProviderId, model_id: crate::ModelId) -> LlmRunOutcome {
    LlmRunOutcome::Cancelled(PartialResponse {
        provider: provider_id,
        model: model_id,
        assistant_rounds: Vec::new(),
        tool_executions: Vec::new(),
        usage: Usage::default(),
    })
}
