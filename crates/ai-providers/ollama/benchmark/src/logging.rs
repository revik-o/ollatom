use llm::{RunEvent, SequencedEvent, ToolEvent, ToolOutput};
use std::{
    fs::File,
    io::{self, Write},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct StageLogger {
    log_file: Arc<Mutex<File>>,
}

impl StageLogger {
    #[must_use]
    pub fn new(file: File) -> Self {
        Self {
            log_file: Arc::new(Mutex::new(file)),
        }
    }

    pub fn write_text(&self, text: impl AsRef<str>) -> io::Result<()> {
        let text = text.as_ref();
        let mut standard_output = std::io::stdout().lock();
        standard_output.write_all(text.as_bytes())?;
        standard_output.flush()?;
        let mut log_file = self
            .log_file
            .lock()
            .map_err(|_| io::Error::other("stage log lock poisoned"))?;
        log_file.write_all(text.as_bytes())?;
        log_file.flush()
    }

    pub fn write_line(&self, line: impl AsRef<str>) -> io::Result<()> {
        self.write_text(format!("{}\n", line.as_ref()))
    }

    pub fn write_event(&self, sequenced_event: &SequencedEvent) -> io::Result<()> {
        let event_data =
            serde_json::to_value(&sequenced_event.event).map_err(event_serialization_error)?;
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
            RunEvent::Tool(ToolEvent::Finished { output }) => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event": "tool_finished",
                "tool_name": output.content.get("tool").and_then(serde_json::Value::as_str),
                "call_id": output.call_id,
                "output": output.content,
                "error": output.error,
                "event_data": event_data,
            }),
            _ => serde_json::json!({
                "run_id": sequenced_event.run_id,
                "sequence": sequenced_event.sequence,
                "event_data": event_data,
            }),
        };
        self.write_line(
            serde_json::to_string(&serialized_event).map_err(event_serialization_error)?,
        )
    }
}

#[derive(Default)]
pub struct ModelToolActivity {
    invocation_count: usize,
    successful_build_count: usize,
}

impl ModelToolActivity {
    pub fn record(&mut self, event: &RunEvent) {
        match event {
            RunEvent::Tool(ToolEvent::Started { .. }) => self.invocation_count += 1,
            RunEvent::Tool(ToolEvent::Finished { output })
                if tool_output_is_successful_build(output) =>
            {
                self.successful_build_count += 1;
            }
            _ => {}
        }
    }

    pub fn require_successful_agent_stage(&self, stage_number: u8) -> Result<(), String> {
        if self.invocation_count == 0 {
            return Err(format!("stage {stage_number} had no model tool invocation"));
        }

        if self.successful_build_count == 0 {
            return Err(format!(
                "stage {stage_number} had no successful model build"
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn invocation_count(&self) -> usize {
        self.invocation_count
    }

    #[must_use]
    pub fn successful_build_count(&self) -> usize {
        self.successful_build_count
    }
}

fn tool_output_is_successful_build(tool_output: &ToolOutput) -> bool {
    tool_output.error.is_none()
        && tool_output
            .content
            .get("result")
            .and_then(|result| result.get("succeeded"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        && tool_output
            .content
            .get("tool")
            .and_then(serde_json::Value::as_str)
            == Some("execute_command")
        && tool_output
            .content
            .get("result")
            .and_then(|result| result.get("command"))
            .and_then(serde_json::Value::as_str)
            == Some("npm run build")
}

fn event_serialization_error(error: serde_json::Error) -> io::Error {
    io::Error::other(format!("event serialization failed: {error}"))
}
