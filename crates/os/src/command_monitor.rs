use super::{
    command::{CommandError, CommandOutcome, TerminalLogs},
    command_output::{TerminalOutputDestination, spawn_output_reader},
    command_process::{join_output_reader, snapshot_terminal_logs, wait_for_process},
};
use std::{
    io::Read,
    process::Child,
    sync::atomic::AtomicBool,
    sync::{Arc, Mutex},
    thread,
};
use tokio::sync::oneshot;

#[derive(Clone)]
pub(super) struct SharedCommandExecution {
    pub(super) child_process: Arc<Mutex<Option<Child>>>,
    pub(super) terminal_logs: Arc<Mutex<TerminalLogs>>,
    pub(super) terminated_by_request: Arc<AtomicBool>,
}

impl SharedCommandExecution {
    pub(super) fn new(child_process: Child) -> Self {
        Self {
            child_process: Arc::new(Mutex::new(Some(child_process))),
            terminal_logs: Arc::new(Mutex::new(TerminalLogs::default())),
            terminated_by_request: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub(super) fn spawn_command_monitor(
    command: String,
    shared_execution: SharedCommandExecution,
    standard_output: Option<impl Read + Send + 'static>,
    standard_error: Option<impl Read + Send + 'static>,
    sender: oneshot::Sender<Result<CommandOutcome, CommandError>>,
) {
    thread::spawn(move || {
        let standard_output_reader = spawn_output_reader(
            standard_output,
            shared_execution.terminal_logs.clone(),
            TerminalOutputDestination::StandardOutput,
        );
        let standard_error_reader = spawn_output_reader(
            standard_error,
            shared_execution.terminal_logs.clone(),
            TerminalOutputDestination::StandardError,
        );
        let process_status = wait_for_process(&shared_execution.child_process);
        let standard_output_error = join_output_reader(standard_output_reader);
        let standard_error_error = join_output_reader(standard_error_reader);
        let output_error = standard_output_error.or(standard_error_error);
        let result = match (process_status, output_error) {
            (Ok(process_status), None) => {
                let terminated = shared_execution
                    .terminated_by_request
                    .load(std::sync::atomic::Ordering::Acquire);
                Ok(CommandOutcome {
                    command,
                    exit_code: process_status.code(),
                    succeeded: process_status.success(),
                    terminated,
                    terminal_logs: snapshot_terminal_logs(&shared_execution.terminal_logs),
                })
            }
            (Err(error), _) | (_, Some(error)) => Err(error),
        };
        let _ = sender.send(result);
    });
}
