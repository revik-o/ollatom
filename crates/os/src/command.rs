use super::{
    command_monitor::{SharedCommandExecution, spawn_command_monitor},
    command_process::{ProcessTerminationResult, terminate_child_process},
};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    path::Path,
    pin::Pin,
    process::{Command, Stdio},
    sync::atomic::Ordering,
    task::{Context, Poll},
};
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CommandError {
    #[error("command must contain an executable")]
    EmptyCommand,
    #[error("command arguments could not be parsed")]
    InvalidCommand,
    #[error("command could not be started: {0}")]
    Spawn(String),
    #[error("command output failed: {0}")]
    Output(String),
    #[error("command completion was disconnected")]
    CompletionDisconnected,
    #[error("command termination failed: {0}")]
    Termination(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct TerminalLogs {
    pub standard_output: String,
    pub standard_error: String,
}

impl TerminalLogs {
    #[must_use]
    pub fn combined(&self) -> String {
        let mut combined = String::new();
        combined.push_str(&self.standard_output);
        combined.push_str(&self.standard_error);
        combined
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct CommandOutcome {
    pub command: String,
    pub exit_code: Option<i32>,
    pub succeeded: bool,
    pub terminated: bool,
    pub terminal_logs: TerminalLogs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct CommandInvocation {
    pub program: String,
    pub arguments: Vec<String>,
}

impl CommandOutcome {
    pub fn terminal_logs(&self) -> &TerminalLogs {
        &self.terminal_logs
    }

    pub fn get_terminal_logs(&self) -> &TerminalLogs {
        self.terminal_logs()
    }
}

#[must_use]
pub struct CommandEntity {
    command: String,
    shared_execution: SharedCommandExecution,
    completion: Option<oneshot::Receiver<Result<CommandOutcome, CommandError>>>,
}

pub fn execute_command(command: impl AsRef<str>) -> Result<CommandEntity, CommandError> {
    execute_command_in_directory(".", command)
}

pub fn execute_command_in_directory(
    working_directory: impl AsRef<Path>,
    command: impl AsRef<str>,
) -> Result<CommandEntity, CommandError> {
    let command_invocation = parse_command(command)?;

    execute_command_in_directory_with_arguments(
        working_directory,
        command_invocation.program,
        command_invocation.arguments,
    )
}

pub fn parse_command(command: impl AsRef<str>) -> Result<CommandInvocation, CommandError> {
    let command = command.as_ref().trim();
    let command_parts = shlex::split(command).ok_or(CommandError::InvalidCommand)?;
    let Some((program, arguments)) = command_parts.split_first() else {
        return Err(CommandError::EmptyCommand);
    };

    Ok(CommandInvocation {
        program: program.clone(),
        arguments: arguments.to_vec(),
    })
}

pub fn execute_command_in_directory_with_arguments(
    working_directory: impl AsRef<Path>,
    program: impl AsRef<str>,
    arguments: impl IntoIterator<Item = String>,
) -> Result<CommandEntity, CommandError> {
    let program = program.as_ref().trim();

    if program.is_empty() {
        return Err(CommandError::EmptyCommand);
    }

    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let command_parts = std::iter::once(program)
        .chain(arguments.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let command = shlex::try_join(command_parts).map_err(|_| CommandError::InvalidCommand)?;
    let mut process_command = Command::new(program);
    process_command
        .args(arguments)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::command_process::configure_process_group(&mut process_command);
    let mut child_process = process_command
        .spawn()
        .map_err(|error| CommandError::Spawn(error.to_string()))?;
    let standard_output = child_process.stdout.take();
    let standard_error = child_process.stderr.take();
    let shared_execution = SharedCommandExecution::new(child_process);
    let (sender, receiver) = oneshot::channel();

    spawn_command_monitor(
        command.clone(),
        shared_execution.clone(),
        standard_output,
        standard_error,
        sender,
    );

    Ok(CommandEntity {
        command,
        shared_execution,
        completion: Some(receiver),
    })
}

impl CommandEntity {
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn terminal_logs(&self) -> TerminalLogs {
        super::command_process::snapshot_terminal_logs(&self.shared_execution.terminal_logs)
    }

    pub fn get_terminal_logs(&self) -> TerminalLogs {
        self.terminal_logs()
    }

    pub fn terminate(&self) -> Result<(), CommandError> {
        let mut child_process_guard = self
            .shared_execution
            .child_process
            .lock()
            .map_err(|_| CommandError::Termination("process state lock was poisoned".into()))?;

        if let Some(child_process) = child_process_guard.as_mut() {
            let termination_result = terminate_child_process(child_process)
                .map_err(|error| CommandError::Termination(error.to_string()))?;

            if termination_result == ProcessTerminationResult::SignalSent {
                self.shared_execution
                    .terminated_by_request
                    .store(true, Ordering::Release);
            }
        }

        Ok(())
    }
}

impl Drop for CommandEntity {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

impl Future for CommandEntity {
    type Output = Result<CommandOutcome, CommandError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(completion_receiver) = self.completion.as_mut() else {
            return Poll::Ready(Err(CommandError::CompletionDisconnected));
        };

        match Pin::new(completion_receiver).poll(context) {
            Poll::Ready(Ok(result)) => {
                self.completion.take();
                Poll::Ready(result)
            }
            Poll::Ready(Err(_)) => {
                self.completion.take();
                Poll::Ready(Err(CommandError::CompletionDisconnected))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
