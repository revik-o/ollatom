use llm::{
    ContextOptions, ContextOverflowPolicy, GenerationOptions, LlmError, LlmOptions, LlmRunOutcome,
    LocalRuntimeOptions, OptionHandlingMode, ReasoningEffort, RunLimits, TransportOptions,
};
use ollama_benchmark::BenchmarkArtifacts;
use std::{error::Error, sync::Arc, time::Duration};

mod stages;
mod summary;

pub(crate) const DEFAULT_MODEL_IDENTIFIER: &str = "gemma4:e2b";
pub(crate) const AGENT_SYSTEM_PROMPT: &str = include_str!("../prompts/system.md");
const STAGE_THREE_TASK_PROMPT: &str = include_str!("../prompts/stage-3.md");
const STAGE_FOUR_TASK_PROMPT: &str = include_str!("../prompts/stage-4.md");
const STAGE_FIVE_TASK_PROMPT: &str = include_str!("../prompts/stage-5.md");
pub(crate) const AGENT_STAGE_PROMPTS: [(u8, &str); 3] = [
    (3, STAGE_THREE_TASK_PROMPT),
    (4, STAGE_FOUR_TASK_PROMPT),
    (5, STAGE_FIVE_TASK_PROMPT),
];
pub(crate) const AVAILABLE_AGENT_TOOL_NAMES: [&str; 7] = [
    "list_files",
    "read_file",
    "write_file",
    "create_directory",
    "rename_path",
    "delete_path",
    "execute_command",
];
pub(crate) const AGENT_COMMAND_TIMEOUT_MILLISECONDS: u64 = 15 * 60 * 1_000;
pub(crate) const AGENT_COMMAND_TIMEOUT: Duration =
    Duration::from_millis(AGENT_COMMAND_TIMEOUT_MILLISECONDS);
const MODEL_KEEP_ALIVE_SECONDS: u64 = 15 * 60;
pub(crate) type BenchmarkResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> BenchmarkResult<()> {
    let artifacts = Arc::new(BenchmarkArtifacts::from_current_executable()?);
    artifacts.initialize()?;
    let model_identifier =
        std::env::var("OLLAMA_BENCHMARK_MODEL").unwrap_or_else(|_| DEFAULT_MODEL_IDENTIFIER.into());
    let ollama_provider = ollama::OllamaProvider::from_environment()?;
    let discovery_runtime = llm::LlmRuntime::builder()
        .provider_with_default(Arc::new(ollama_provider.clone()), model_identifier.clone())
        .build()?;
    stages::run_discovery_stages(&discovery_runtime, &model_identifier, &artifacts).await?;
    artifacts.prepare_project_for_generation()?;
    let tool_registry = llm::register_basic_tools(
        llm::BasicToolConfiguration::new(artifacts.project_root())
            .command_timeout(AGENT_COMMAND_TIMEOUT)
            .exclude_directory("node_modules")
            .exclude_directory("dist"),
    )?;
    let agent_runtime = llm::LlmRuntime::builder()
        .provider_with_default(Arc::new(ollama_provider), model_identifier.clone())
        .tools(tool_registry)
        .build()?;
    stages::run_all_agent_stages(&agent_runtime, &model_identifier, &artifacts).await
}

pub(crate) fn require_completed_outcome(
    run_outcome: Result<LlmRunOutcome, LlmError>,
    stage_number: u8,
) -> BenchmarkResult<()> {
    match run_outcome {
        Ok(LlmRunOutcome::Completed(_)) => Ok(()),
        Ok(LlmRunOutcome::Cancelled(_)) => {
            Err(format!("stage {stage_number} LLM run was cancelled").into())
        }
        Err(error) => Err(format!("stage {stage_number} LLM run failed: {error}").into()),
    }
}

pub(crate) fn benchmark_options() -> LlmOptions {
    LlmOptions {
        context: ContextOptions {
            input_token_budget: None,
            overflow_policy: Some(ContextOverflowPolicy::TruncateOldest),
            optimization: true,
        },
        generation: GenerationOptions {
            max_output_tokens: None,
            temperature: Some(0.2),
            max_token_choices: Some(40),
            diversity_threshold: Some(0.9),
            repeat_penalty: None,
            stop_sequences: None,
        },
        reasoning: llm::ReasoningOptions {
            effort: ReasoningEffort::Low,
            budget_tokens: None,
        },
        local: LocalRuntimeOptions {
            context_size: Some(65_536),
            evaluation_batch_size: None,
            threads: Some(4),
            keep_alive_seconds: Some(MODEL_KEEP_ALIVE_SECONDS),
            required_phase: None,
        },
        transport: TransportOptions {
            connect_timeout_ms: Some(10_000),
            stream_idle_timeout_ms: Some(900_000),
            overall_timeout_ms: Some(3_600_000),
        },
        handling: OptionHandlingMode::BestEffort,
    }
}

pub(crate) fn benchmark_limits() -> RunLimits {
    RunLimits {
        provider_rounds: 128,
        total_tool_calls: 512,
        tool_calls_per_round: 16,
        child_subagents: 0,
        subagent_depth: 0,
        tool_timeout_ms: AGENT_COMMAND_TIMEOUT_MILLISECONDS,
    }
}
