use crate::{
    LlmError, LlmRunOutcome, PartialResponse, ProviderRunOutcome, ToolExecutionRecord, Usage,
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
        ProviderRunOutcome::Completed(response) => {
            (&mut response.usage, &mut response.tool_executions)
        }
        ProviderRunOutcome::Cancelled(partial_response) => (
            &mut partial_response.usage,
            &mut partial_response.tool_executions,
        ),
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

            ProviderRunOutcome::Cancelled(partial_response) => {
                LlmRunOutcome::Cancelled(partial_response)
            }
        },
        Err(_) => {
            let streamed_output = event_dispatcher.streamed_output().await;
            let mut usage = streamed_output.usage;
            usage.child_usage.extend(host_results.child_usage);

            LlmRunOutcome::Cancelled(PartialResponse {
                text: streamed_output.text,
                visible_reasoning: (!streamed_output.visible_reasoning.is_empty())
                    .then_some(streamed_output.visible_reasoning),
                provider: provider_id,
                model: model_id,
                tool_executions: host_results.tool_executions,
                usage,
            })
        }
    }
}

pub(super) fn empty_cancelled_outcome(
    provider_id: crate::ProviderId,
    model_id: crate::ModelId,
) -> LlmRunOutcome {
    LlmRunOutcome::Cancelled(PartialResponse {
        text: String::new(),
        visible_reasoning: None,
        provider: provider_id,
        model: model_id,
        tool_executions: Vec::new(),
        usage: Usage::default(),
    })
}
