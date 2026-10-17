use crate::MAXIMUM_UNAVAILABLE_BUILD_ATTEMPTS;
use llm::{RunEvent, SequencedEvent, ToolEvent, ToolFailure, ToolOutput};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, BufWriter, Write},
    sync::{Arc, LazyLock, Mutex},
};

static STANDARD_OUTPUT: LazyLock<Mutex<BufWriter<io::Stdout>>> =
    LazyLock::new(|| Mutex::new(BufWriter::new(io::stdout())));

#[derive(Clone)]
pub struct StageLogger {
    log_file: Arc<Mutex<BufWriter<File>>>,
}

impl StageLogger {
    #[must_use]
    pub fn new(file: File) -> Self {
        Self {
            log_file: Arc::new(Mutex::new(BufWriter::new(file))),
        }
    }

    pub fn write_text(&self, text: impl AsRef<str>) -> io::Result<()> {
        let text = text.as_ref();
        let mut standard_output = STANDARD_OUTPUT
            .lock()
            .map_err(|_| io::Error::other("standard output lock poisoned"))?;

        standard_output.write_all(text.as_bytes())?;

        let mut log_file = self
            .log_file
            .lock()
            .map_err(|_| io::Error::other("stage log lock poisoned"))?;

        log_file.write_all(text.as_bytes())
    }

    pub fn flush(&self) -> io::Result<()> {
        STANDARD_OUTPUT
            .lock()
            .map_err(|_| io::Error::other("standard output lock poisoned"))?
            .flush()?;
        self.log_file
            .lock()
            .map_err(|_| io::Error::other("stage log lock poisoned"))?
            .flush()
    }

    pub fn write_line(&self, line: impl AsRef<str>) -> io::Result<()> {
        self.write_text(format!("{}\n", line.as_ref()))
    }

    pub fn write_event(&self, sequenced_event: &SequencedEvent) -> io::Result<()> {
        let event_data = serde_json::to_value(&sequenced_event.event).map_err(event_serialization_error)?;
        let serialized_event = match &sequenced_event.event {
            RunEvent::Tool(ToolEvent::Planned { call }) => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event": "tool_planned",
                "tool_name": call.name,
                "call_id": call.id,
                "arguments": call.arguments,
                "event_data": event_data,
            }),
            RunEvent::Tool(ToolEvent::ApprovalRequested { call_id }) => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event": "tool_approval_requested",
                "call_id": call_id,
                "event_data": event_data,
            }),
            RunEvent::Tool(ToolEvent::Started { call }) => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event": "tool_started",
                "tool_name": call.name,
                "call_id": call.id,
                "arguments": call.arguments,
                "event_data": event_data,
            }),
            RunEvent::Tool(ToolEvent::Finished { output }) => {
                let (call_id, content, error) = match output {
                    ToolOutput::Success { call_id, content } => (call_id, content.clone(), serde_json::Value::Null),
                    ToolOutput::Failure { call_id, error } => (
                        call_id,
                        serde_json::Value::Null,
                        serde_json::to_value(error).map_err(event_serialization_error)?,
                    ),
                };

                serde_json::json!({
                    "run_id": sequenced_event.run_id,
                    "sequence": sequenced_event.sequence,
                    "event": "tool_finished",
                    "tool_name": content.get("tool").and_then(serde_json::Value::as_str),
                    "call_id": call_id,
                    "output": content,
                    "error": error,
                    "event_data": event_data,
                })
            }
            _ => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event_data": event_data,
            }),
        };
        self.write_line(serde_json::to_string(&serialized_event).map_err(event_serialization_error)?)
    }
}

#[derive(Default)]
pub struct ModelToolActivity {
    invocation_count: usize,
    successful_build_count: usize,
    node_command_attempt_count: usize,
    unavailable_node_command_attempt_count: usize,
    node_command_call_identifiers: BTreeSet<String>,
    finished_node_command_call_identifiers: BTreeSet<String>,
    node_command_programs: BTreeMap<String, String>,
    build_call_identifiers: BTreeSet<String>,
    finished_build_call_identifiers: BTreeSet<String>,
    rustc_version_call_identifiers: BTreeSet<String>,
    cargo_version_call_identifiers: BTreeSet<String>,
    rustc_version_output: Option<String>,
    cargo_version_output: Option<String>,
    tool_call_identifiers: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentStageBuildStatus {
    SuccessfulBuild,
    SkippedAfterToolUnavailable,
}

impl ModelToolActivity {
    pub fn record(&mut self, event: &RunEvent) {
        match event {
            RunEvent::Tool(ToolEvent::Planned { call }) | RunEvent::Tool(ToolEvent::Started { call }) => {
                if self.tool_call_identifiers.insert(call.id.clone()) {
                    self.invocation_count = self.invocation_count.saturating_add(1);
                }

                if is_node_command_call(call) {
                    if self.node_command_call_identifiers.insert(call.id.clone()) {
                        self.node_command_attempt_count = self.node_command_attempt_count.saturating_add(1);
                    }

                    if let Some(program) = command_arguments(call).get(2) {
                        self.node_command_programs
                            .insert(call.id.clone(), (*program).to_owned());
                    }
                }

                if is_mise_build_call(call) {
                    self.build_call_identifiers.insert(call.id.clone());
                }

                if is_mise_version_call(call, "rustc") {
                    self.rustc_version_call_identifiers.insert(call.id.clone());
                }

                if is_mise_version_call(call, "cargo") {
                    self.cargo_version_call_identifiers.insert(call.id.clone());
                }
            }
            RunEvent::Tool(ToolEvent::Finished { output }) => {
                if self.node_command_call_identifiers.contains(output.call_id())
                    && self
                        .finished_node_command_call_identifiers
                        .insert(output.call_id().to_owned())
                    && self
                        .node_command_programs
                        .get(output.call_id())
                        .is_some_and(|program| tool_output_indicates_unavailable_tool(output, program))
                {
                    self.unavailable_node_command_attempt_count =
                        self.unavailable_node_command_attempt_count.saturating_add(1);
                }

                if self.build_call_identifiers.contains(output.call_id())
                    && self.finished_build_call_identifiers.insert(output.call_id().to_owned())
                {
                    if tool_output_is_successful_build(output) {
                        self.successful_build_count = self.successful_build_count.saturating_add(1);
                    }
                }

                if self.rustc_version_call_identifiers.contains(output.call_id()) {
                    self.rustc_version_output = tool_output_version(output, "rustc ");
                }

                if self.cargo_version_call_identifiers.contains(output.call_id()) {
                    self.cargo_version_output = tool_output_version(output, "cargo ");
                }
            }
            _ => {}
        }
    }

    pub fn require_successful_agent_stage(&self, stage_number: u8) -> Result<AgentStageBuildStatus, String> {
        if self.invocation_count == 0 {
            return Err(format!("stage {stage_number} had no model tool invocation"));
        }

        if self.successful_build_count > 0 {
            return Ok(AgentStageBuildStatus::SuccessfulBuild);
        }

        if self.node_command_attempt_count >= MAXIMUM_UNAVAILABLE_BUILD_ATTEMPTS
            && self.unavailable_node_command_attempt_count >= MAXIMUM_UNAVAILABLE_BUILD_ATTEMPTS
        {
            return Ok(AgentStageBuildStatus::SkippedAfterToolUnavailable);
        }

        Err(format!(
            "stage {stage_number} had no successful model build ({} Node command attempts, {} tool-unavailable failures; at least {MAXIMUM_UNAVAILABLE_BUILD_ATTEMPTS} of each are required to continue)",
            self.node_command_attempt_count, self.unavailable_node_command_attempt_count
        ))
    }

    pub fn require_rust_toolchain_versions(&self, stage_number: u8) -> Result<(&str, &str), String> {
        let rustc_version = self
            .rustc_version_output
            .as_deref()
            .ok_or_else(|| format!("stage {stage_number} did not print the rustc version through mise"))?;
        let cargo_version = self
            .cargo_version_output
            .as_deref()
            .ok_or_else(|| format!("stage {stage_number} did not print the cargo version through mise"))?;

        Ok((rustc_version, cargo_version))
    }

    #[must_use]
    pub fn invocation_count(&self) -> usize {
        self.invocation_count
    }

    #[must_use]
    pub fn successful_build_count(&self) -> usize {
        self.successful_build_count
    }

    #[must_use]
    pub fn node_command_attempt_count(&self) -> usize {
        self.node_command_attempt_count
    }

    #[must_use]
    pub fn unavailable_node_command_attempt_count(&self) -> usize {
        self.unavailable_node_command_attempt_count
    }
}

fn is_node_command_call(call: &llm::ToolCall) -> bool {
    let arguments = command_arguments(call);
    call.name == "execute_command"
        && command_program(call) == Some("mise")
        && arguments.len() >= 3
        && arguments[0] == "exec"
        && arguments[1] == "--"
        && matches!(arguments[2], "npm" | "npx")
}

fn is_mise_build_call(call: &llm::ToolCall) -> bool {
    let arguments = command_arguments(call);
    is_node_command_call(call)
        && arguments.len() == 5
        && arguments[2] == "npm"
        && arguments[3] == "run"
        && arguments[4] == "build"
}

fn is_mise_version_call(call: &llm::ToolCall, executable: &str) -> bool {
    let arguments = command_arguments(call);
    call.name == "execute_command"
        && command_program(call) == Some("mise")
        && arguments.len() == 4
        && arguments[0] == "exec"
        && arguments[1] == "--"
        && arguments[2] == executable
        && arguments[3] == "--version"
}

fn command_program(call: &llm::ToolCall) -> Option<&str> {
    call.arguments.get("program").and_then(serde_json::Value::as_str)
}

fn command_arguments(call: &llm::ToolCall) -> Vec<&str> {
    call.arguments
        .get("arguments")
        .and_then(serde_json::Value::as_array)
        .map(|arguments| arguments.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default()
}

fn tool_output_is_successful_build(tool_output: &ToolOutput) -> bool {
    tool_output
        .content()
        .and_then(|content| content.get("result"))
        .and_then(|result| result.get("succeeded"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn tool_output_version(tool_output: &ToolOutput, expected_prefix: &str) -> Option<String> {
    (tool_output
        .content()
        .and_then(|content| content.get("result"))
        .and_then(|result| result.get("succeeded"))
        .and_then(serde_json::Value::as_bool)
        == Some(true))
    .then(|| {
        tool_output
            .content()
            .and_then(|content| content.pointer("/result/terminal_logs/standard_output"))
            .and_then(serde_json::Value::as_str)
            .and_then(|output| {
                output
                    .lines()
                    .find(|line| line.starts_with(expected_prefix))
                    .map(str::to_owned)
            })
    })
    .flatten()
}

fn tool_output_indicates_unavailable_tool(tool_output: &ToolOutput, command: &str) -> bool {
    match tool_output {
        ToolOutput::Failure {
            error: ToolFailure::Denied(_),
            ..
        } => true,
        ToolOutput::Failure {
            error: ToolFailure::Execution(message),
            ..
        } => {
            let message = message.to_ascii_lowercase();
            [
                "not found in the trusted path",
                "executable was not found",
                "command could not be started",
                "sandbox is unavailable",
                "not authorized",
            ]
            .iter()
            .any(|marker| message.contains(marker))
        }
        ToolOutput::Failure { .. } => false,
        ToolOutput::Success { content, .. } => {
            if content
                .pointer("/result/succeeded")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                return false;
            }

            let output = [
                content
                    .pointer("/result/terminal_logs/standard_output")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
                content
                    .pointer("/result/terminal_logs/standard_error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            ]
            .join("\n")
            .to_ascii_lowercase();
            command_is_unavailable(&output, command) || command_is_unavailable(&output, "node")
        }
    }
}

fn command_is_unavailable(output: &str, command: &str) -> bool {
    [
        format!("{command}: not found"),
        format!("{command}: command not found"),
        format!("{command}: permission denied"),
        format!("{command}: no such file or directory"),
        format!("'{command}': no such file or directory"),
        format!("/{command}: permission denied"),
        format!("/{command}: no such file or directory"),
        format!("command {command} not found"),
        format!("could not find command {command}"),
        format!("could not find executable {command}"),
        format!("{command} is not installed"),
        format!("{command} is unavailable"),
        format!("no version is set for command {command}"),
        format!("no version set for command {command}"),
        format!("missing installed version for {command}"),
        format!("failed to resolve executable {command}"),
        format!("failed to run {command}: no such file"),
        format!("failed to start {command}: no such file"),
    ]
    .iter()
    .any(|marker| output.contains(marker))
}
fn event_serialization_error(error: serde_json::Error) -> io::Error {
    io::Error::other(format!("event serialization failed: {error}"))
}
