# llm

`llm` contains provider-neutral contracts and runtime services for Ollatom. Concrete provider crates depend on this crate; this crate never imports them.

The public API supports an injected runtime and an optional process-global facade. Requests expose no tools by default. Providers own their native multi-round continuation and use `ProviderRunHost` for ordered events, complete tool-call batches, interactions, and subagents. Shared runtime services enforce limits, policy, sequential execution, duplicate call handling, cancellation, and event ordering.

Every Rust source file in this crate is limited to 270 lines. `tests/source_structure.rs` enforces the limit recursively.

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
    .send();

let run_id = run.id();

match run.await? {
    LlmRunOutcome::Completed(response) => {
        println!("Response: {}", response.text);
        if let Some(reasoning) = response.visible_reasoning {
            println!("Reasoning: {}", reasoning);
        }
        println!("Tokens used: {}", response.usage.total_tokens_including_children());
    },
    LlmRunOutcome::Cancelled(partial) => {
        println!("Run cancelled. Partial response: {}", partial.text);
    }
}
```
**Description:** Demonstrates how to build an LLM request using the global facade. You can specify the target provider, model, system prompt, and allowed tools. Calling `.send()` starts the execution asynchronously, returning an `LlmRun`. Awaiting the `LlmRun` yields the final `LlmRunOutcome` which contains either the completed response and token usage or a cancelled partial response.

### 3. Event Streaming and Policy

```rust
use llm::{Llm, BuiltInProvider, RunEvent, AllowedPermissions};
use std::path::PathBuf;

let mut run = Llm::init(BuiltInProvider::Ollama)
    .user_message("Analyze these files and run the tests")
    .trusted_folders([PathBuf::from("/path/to/project")])
    .allowed(AllowedPermissions::ALL_FILESYSTEM_ACCESS)
    .stream_approval_requests()
    .send();

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

### 4. Custom Tools

```rust
use llm::{
    Tool, ToolDefinition, ToolCall, ToolPlan, ToolOutput, ToolFailure,
    AuthorizationGrant, BoxFuture,
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
            requires_authorization: false,
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
