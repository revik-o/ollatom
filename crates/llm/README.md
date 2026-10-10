# llm

`llm` contains provider-neutral contracts and runtime services for Ollatom. Concrete provider crates depend on this crate; this crate never imports them.

The public API supports an injected runtime and an optional process-global facade. Requests expose no tools by default. Providers own their native multi-round continuation and use `ProviderRunHost` for ordered events, complete tool-call batches, interactions, and subagents. Shared runtime services enforce limits, policy, sequential execution, duplicate call handling, cancellation, and event ordering.

## Public API Overview

### 1. Runtime Initialization and Global Facade

```rust
use llm::{LlmRuntimeBuilder, LlmProvider, ToolRegistry, Llm};
use std::sync::Arc;

let mut tools = ToolRegistry::new();
// tools.register(...)?;

let runtime = LlmRuntimeBuilder::new()
    .provider_with_default(provider, "qwen3:4b")
    .tools(tools)
    .build()?;

// Optionally install the runtime globally to use the `Llm` facade anywhere
Llm::install_global(runtime)?;
```
**Description:** Demonstrates how to configure and build the `LlmRuntime`. You can register providers, provide a tool registry, and configure event sinks or subagent profiles. Installing it globally via `Llm::install_global` allows you to create requests using `Llm::init` without passing around the runtime reference.

### 2. Building and Executing Requests

```rust
use llm::{Llm, BuiltInProvider, ReasoningEffort, LlmRunOutcome};

let run = Llm::init(BuiltInProvider::Ollama)
    .model("qwen3:4b")
    .effort(ReasoningEffort::Medium)
    .system_prompt("You are a helpful assistant.")
    .tools(["read_file", "ask_user"])
    .user_message("Inspect the project")
    .send()?;

let run_id = run.id();

match run.await? {
    LlmRunOutcome::Completed(response) => {
        for assistant_round in &response.assistant_rounds {
            println!("Round {}: {}", assistant_round.round_number, assistant_round.text);
            if let Some(reasoning) = &assistant_round.visible_reasoning {
                println!("Reasoning: {}", reasoning);
            }
        }
        println!("Tokens used: {}", response.usage.total_tokens_including_children()?);
    },
    LlmRunOutcome::Cancelled(partial) => {
        println!("Run cancelled after {} assistant rounds.", partial.assistant_rounds.len());
    }
    LlmRunOutcome::Failed(failed) => {
        println!("Run failed: {}", failed.error);
        println!("Preserved {} assistant rounds.", failed.partial_response.assistant_rounds.len());
    }
}
```
**Description:** Demonstrates how to build an LLM request using the global facade. You can specify the target provider, model, system prompt, and allowed tools. Calling `.send()` validates that a Tokio runtime is available, starts execution immediately, and returns `Result<LlmRun, LlmError>`. Awaiting the `LlmRun` yields the final `LlmRunOutcome`: a completed response, a cancelled partial response, or a `FailedRun` with its error and preserved assistant rounds. Each assistant round retains its own text, reasoning, tool calls, finish reason, and usage. `FailedRun.terminal_event_delivered` reports whether the terminal event reached the dispatcher and configured sink.

### 3. Transport Deadlines

`TransportOptions.overall_timeout_ms` sets the run deadline. `event_delivery_timeout_ms` bounds a single event-sink write and time spent waiting for the event dispatcher; it defaults to 5,000 ms. Each event operation uses the shorter of that value and the remaining run deadline. A zero timeout is rejected. Timeouts after provider startup produce `FailedRun` and retain available partial output and completed tool records.

### 4. Event Streaming and Policy

```rust
use llm::{Llm, BuiltInProvider, RunEvent, AllowedPermissions};
use std::path::PathBuf;

let mut run = Llm::init(BuiltInProvider::Ollama)
    .user_message("Analyze these files and run the tests")
    .trusted_folders([PathBuf::from("/path/to/project")])
    .allowed(AllowedPermissions::ALL_FILESYSTEM_ACCESS)
    .stream_approval_requests()
    .send()?;

let mut stream = run.take_event_stream().expect("stream available");

while let Some(sequenced_event) = stream.next().await {
    match sequenced_event.event {
        RunEvent::ResponseDelta(text) => print!("{}", text),
        RunEvent::Tool(tool_event) => println!("Tool event: {:?}", tool_event),
        RunEvent::Completed => println!("\nRun finished."),
        _ => {}
    }
}

let outcome = run.await?;
```
**Description:** `LlmRun` provides an event stream to observe the execution in real time. This is useful for streaming text deltas to a user interface, observing tool execution states, and tracking usage metrics. Additionally, this shows configuring `RunPolicy` parameters like `trusted_folders` and `AllowedPermissions` to constrain what tools can do during execution.

### 5. Custom Tools

```rust
use llm::{
    Tool, ToolDefinition, ToolCall, ToolPlan, ToolOutput, ToolFailure,
    AuthorizationGrant, BoxFuture, ToolAuthorizationRequirement,
};
use serde_json::json;

struct GreeterTool;

impl Tool for GreeterTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "greet".into(),
            description: "Greets the user".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
            authorization_requirement: ToolAuthorizationRequirement::CapabilityPolicy,
        }
    }

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        Ok(ToolPlan {
            call: call.clone(),
            normalized_arguments: call.arguments.clone(),
            required_capabilities: vec![],
            timeout: None,
        })
    }

    fn execute(
        &self,
        plan: ToolPlan,
        _grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        Box::pin(async move {
            let name = plan.normalized_arguments["name"].as_str().unwrap_or("World");
            Ok(ToolOutput::success(plan.call.id, json!(format!("Hello, {}!", name))))
        })
    }
}
```
**Description:** Demonstrates implementing the `Tool` trait to create custom capabilities for the LLM. Tools define their own JSON schema for arguments, a `plan` phase for normalization and capability checking, and an `execute` phase for the actual logic.

`ToolOutput` is a tagged enum with `Success { call_id, content }` and
`Failure { call_id, error }` variants. A result cannot carry both success
content and a failure. `append_tool_round_to_history` validates one output per
unique tool call and appends results in call order.
