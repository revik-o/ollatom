use super::{
    AGENT_STAGE_PROMPTS, AGENT_SYSTEM_PROMPT, AVAILABLE_AGENT_TOOL_NAMES, BenchmarkResult,
    benchmark_limits, benchmark_options, require_completed_outcome, summary::run_outcome_summary,
};
use llm::ConversationMessage;
use ollama_benchmark::{
    BenchmarkArtifacts, ConversationHistoryRecorder, ModelToolActivity, StageLogger,
    verify_independent_build,
};
use serde_json::json;

pub(crate) async fn run_all_agent_stages(
    agent_runtime: &llm::LlmRuntime,
    model_identifier: &str,
    artifacts: &BenchmarkArtifacts,
) -> BenchmarkResult<()> {
    let mut conversation_history = Vec::new();

    for (stage_number, stage_prompt) in AGENT_STAGE_PROMPTS {
        let completed_stage_history = run_agent_stage(
            agent_runtime,
            model_identifier,
            artifacts,
            stage_number,
            stage_prompt,
            &conversation_history,
        )
        .await?;
        conversation_history.extend(completed_stage_history);
    }

    Ok(())
}

pub(crate) async fn run_discovery_stages(
    discovery_runtime: &llm::LlmRuntime,
    model_identifier: &str,
    artifacts: &BenchmarkArtifacts,
) -> BenchmarkResult<()> {
    let stage_one_logger = StageLogger::new(artifacts.open_stage_log(1)?);
    let mut available_models = discovery_runtime
        .request(llm::BuiltInProvider::Ollama)
        .get_all_models()
        .await?;
    available_models.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    stage_one_logger.write_line("stage 1: available models")?;

    for model_information in &available_models {
        stage_one_logger.write_line(model_information.id.as_str())?;
    }

    if !available_models
        .iter()
        .any(|model_information| model_information.id.as_str() == model_identifier)
    {
        return Err(format!("configured model is unavailable: {model_identifier}").into());
    }

    let stage_two_logger = StageLogger::new(artifacts.open_stage_log(2)?);
    let model_information = discovery_runtime
        .request(llm::BuiltInProvider::Ollama)
        .model(model_identifier)
        .get_info()
        .await?;
    stage_two_logger.write_line("stage 2: model information")?;
    stage_two_logger.write_line(serde_json::to_string_pretty(&model_information)?)?;

    Ok(())
}

async fn run_agent_stage(
    agent_runtime: &llm::LlmRuntime,
    model_identifier: &str,
    artifacts: &BenchmarkArtifacts,
    stage_number: u8,
    stage_prompt: &str,
    conversation_history: &[ConversationMessage],
) -> BenchmarkResult<Vec<ConversationMessage>> {
    let stage_logger = StageLogger::new(artifacts.open_stage_log(stage_number)?);
    stage_logger.write_line(serde_json::to_string(
        &json!({"stage": stage_number, "prompt": stage_prompt, "context": conversation_history}),
    )?)?;

    let mut llm_run = agent_runtime
        .request(llm::BuiltInProvider::Ollama)
        .model(model_identifier)
        .options(benchmark_options())
        .limits(benchmark_limits())
        .system_prompt(AGENT_SYSTEM_PROMPT)
        .context(conversation_history.to_vec())
        .tools(AVAILABLE_AGENT_TOOL_NAMES)
        .trusted_folder_grants([llm::TrustedFolder::full_access(artifacts.project_root())])
        .trusted_commands(["npm(?: .*)?"])
        .deny_untrusted()
        .user_message(stage_prompt)
        .send();

    let mut event_stream = llm_run
        .take_event_stream()
        .ok_or_else(|| "LLM run did not provide an event stream".to_string())?;

    let mut run_future = Box::pin(llm_run);
    let mut run_outcome = None;
    let mut event_stream_ended = false;
    let mut model_tool_activity = ModelToolActivity::default();
    let mut conversation_history_recorder = ConversationHistoryRecorder::default();

    while !event_stream_ended || run_outcome.is_none() {
        tokio::select! {
            llm_run_result = &mut run_future,
            if run_outcome.is_none() => run_outcome = Some(llm_run_result),
            next_event = event_stream.next(), if !event_stream_ended => {
                match next_event {
                    Some(sequenced_event) => {
                        model_tool_activity.record(&sequenced_event.event);
                        conversation_history_recorder.record(&sequenced_event.event);
                        stage_logger.write_event(&sequenced_event)?;
                    }
                    None => event_stream_ended = true,
                }
            }
        }
    }

    let run_outcome = run_outcome.ok_or_else(|| "LLM run ended without an outcome".to_string())?;
    stage_logger.write_line(serde_json::to_string(
        &json!({"stage": stage_number, "outcome": run_outcome_summary(&run_outcome)}),
    )?)?;

    require_completed_outcome(run_outcome, stage_number)?;

    model_tool_activity
        .require_successful_agent_stage(stage_number)
        .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;

    if let Err(error) = verify_independent_build(artifacts, &stage_logger).await {
        stage_logger.write_line(serde_json::to_string(&json!({
            "stage": stage_number,
            "verification": "failed",
            "error": error.to_string()
        }))?)?;

        return Err(error);
    }

    Ok(conversation_history_recorder.into_conversation_history(stage_prompt))
}
