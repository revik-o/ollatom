use super::command::CommandError;
use std::{
    io::ErrorKind,
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProcessTerminationResult {
    SignalSent,
    AlreadyExited,
}

pub(super) fn terminate_child_process(
    child_process: &mut Child,
) -> std::io::Result<ProcessTerminationResult> {
    if child_process.try_wait()?.is_some() {
        return Ok(ProcessTerminationResult::AlreadyExited);
    }

    terminate_running_child_process(child_process)
}

#[cfg(unix)]
pub(super) fn configure_process_group(process_command: &mut Command) {
    use std::os::unix::process::CommandExt;

    process_command.process_group(0);
}

#[cfg(not(unix))]
pub(super) fn configure_process_group(process_command: &mut Command) {
    let _ = process_command;
}

#[cfg(unix)]
fn terminate_running_child_process(
    child_process: &mut Child,
) -> std::io::Result<ProcessTerminationResult> {
    let process_group_identifier = i32::try_from(child_process.id()).map_err(|_| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            "child process identifier exceeds the supported range",
        )
    })?;

    if signal_process_group(process_group_identifier) == 0 {
        return Ok(ProcessTerminationResult::SignalSent);
    }

    let termination_error = std::io::Error::last_os_error();

    if termination_error.raw_os_error() == Some(libc::ESRCH) && child_process.try_wait()?.is_some()
    {
        return Ok(ProcessTerminationResult::AlreadyExited);
    }

    Err(termination_error)
}

#[cfg(unix)]
fn signal_process_group(process_group_identifier: i32) -> i32 {
    unsafe { libc::kill(-process_group_identifier, libc::SIGKILL) }
}

#[cfg(not(unix))]
fn terminate_running_child_process(
    child_process: &mut Child,
) -> std::io::Result<ProcessTerminationResult> {
    match child_process.kill() {
        Ok(()) => Ok(ProcessTerminationResult::SignalSent),
        Err(error) if error.kind() == ErrorKind::InvalidInput => {
            Ok(ProcessTerminationResult::AlreadyExited)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn wait_for_process(
    child_process: &Arc<Mutex<Option<Child>>>,
) -> Result<std::process::ExitStatus, CommandError> {
    loop {
        let process_status = child_process
            .lock()
            .map_err(|_| CommandError::Output("process state lock was poisoned".into()))?
            .as_mut()
            .ok_or_else(|| CommandError::Output("process state was lost".into()))?
            .try_wait()
            .map_err(|error| CommandError::Output(error.to_string()))?;

        if let Some(process_status) = process_status {
            return Ok(process_status);
        }

        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

pub(super) fn join_output_reader(
    reader: Option<thread::JoinHandle<Result<(), CommandError>>>,
) -> Option<CommandError> {
    reader.and_then(|reader| match reader.join() {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(CommandError::Output("output reader thread panicked".into())),
    })
}

pub(super) fn snapshot_terminal_logs(
    terminal_logs: &Arc<Mutex<super::command::TerminalLogs>>,
) -> super::command::TerminalLogs {
    terminal_logs.lock().map_or_else(
        |poisoned| poisoned.into_inner().clone(),
        |logs| logs.clone(),
    )
}
