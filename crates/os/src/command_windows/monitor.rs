use super::super::{
    CommandError, CommandOutcome, MAXIMUM_STANDARD_ERROR_BYTES, MAXIMUM_STANDARD_OUTPUT_BYTES,
    OUTPUT_DRAIN_TIMEOUT, OutputDestination, OutputMessage, SandboxNetworkPolicy,
    SandboxedCommandAuthorization, TerminalLogs, append_output, command_semaphore,
    join_output_reader, output_drain_deadline, spawn_output_reader,
};
use super::{WorkspacePackageAccess, environment};
use rappct::launch::{JobLimits, LaunchOptions, StdioConfig, launch_in_container_with_io};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};

pub(super) async fn monitor_command(
    command_text: String,
    authorization: SandboxedCommandAuthorization,
    terminal_logs: Arc<Mutex<TerminalLogs>>,
    mut termination_receiver: watch::Receiver<bool>,
) -> Result<CommandOutcome, CommandError> {
    let concurrency_permit = tokio::select! {
        biased;
        termination_change = termination_receiver.changed() => {
            let _ = termination_change;
            return Ok(CommandOutcome {
                command: command_text,
                exit_code: None,
                succeeded: false,
                terminated: true,
                terminal_logs: TerminalLogs::default(),
            });
        }
        concurrency_permit = command_semaphore().clone().acquire_owned() => {
            concurrency_permit.map_err(|_| CommandError::CompletionDisconnected)?
        }
    };

    let pinned_workspace = super::super::open_authorized_workspace(&authorization)?;
    let pinned_executable = super::super::open_authorized_executable(&authorization)?;

    let (mut workspace_access, security_capabilities) = WorkspacePackageAccess::grant(
        &authorization.filesystem_policy.workspace_directory,
        &authorization
            .filesystem_policy
            .additional_read_only_directories,
        authorization.network_policy,
    )?;

    super::super::validate_pinned_workspace_path(&authorization, &pinned_workspace)?;
    super::super::validate_pinned_executable_path(&authorization, &pinned_executable)?;

    let command_line = environment::build_windows_command_line(
        &authorization.executable_path,
        &authorization.arguments,
    )?;

    let environment =
        environment::build_windows_environment(&authorization.environment, &authorization)?;

    let launch_options = LaunchOptions {
        exe: authorization.executable_path.clone(),
        cmdline: Some(command_line),
        cwd: Some(authorization.filesystem_policy.workspace_directory.clone()),
        env: Some(environment),
        stdio: StdioConfig::Pipe,
        suspended: true,
        join_job: Some(JobLimits {
            memory_bytes: None,
            cpu_rate_percent: None,
            kill_on_job_close: true,
        }),
        startup_timeout: None,
    };

    let mut launched_process = launch_in_container_with_io(&security_capabilities, &launch_options)
        .map_err(|error| CommandError::Spawn(error.to_string()))?;

    drop(launched_process.stdin.take());

    let mut process_tree_guard = Some(launched_process.job_guard.take().ok_or_else(|| {
        CommandError::Spawn("Windows sandbox did not create a kill-on-close job".into())
    })?);

    let process_identifier = launched_process.pid;

    tokio::task::spawn_blocking(move || subc_jobobject::resume_main_thread(process_identifier))
        .await
        .map_err(|error| CommandError::Spawn(error.to_string()))?
        .map_err(|error| CommandError::Spawn(error.to_string()))?;

    let standard_output = launched_process
        .stdout
        .take()
        .ok_or_else(|| CommandError::Output("standard output pipe was unavailable".into()))?;

    let standard_error = launched_process
        .stderr
        .take()
        .ok_or_else(|| CommandError::Output("standard error pipe was unavailable".into()))?;

    let process_wait = tokio::task::spawn_blocking(move || launched_process.wait(None));

    tokio::pin!(process_wait);

    let (output_sender, mut output_receiver) = mpsc::channel(64);

    let standard_output_reader = spawn_output_reader(
        tokio::fs::File::from_std(standard_output),
        OutputDestination::StandardOutput,
        output_sender.clone(),
    );

    let standard_error_reader = spawn_output_reader(
        tokio::fs::File::from_std(standard_error),
        OutputDestination::StandardError,
        output_sender,
    );

    let mut closed_stream_count = 0_usize;
    let mut standard_output_byte_count = 0_usize;
    let mut standard_error_byte_count = 0_usize;
    let mut combined_output_byte_count = 0_usize;
    let mut process_exit_code = None;
    let mut terminated = false;
    let mut termination_requested = false;
    let mut output_failure = None;

    let output_drain_timeout = tokio::time::sleep(OUTPUT_DRAIN_TIMEOUT);

    tokio::pin!(output_drain_timeout);

    while process_exit_code.is_none() || closed_stream_count < 2 {
        tokio::select! {
            biased;
            process_result = &mut process_wait, if process_exit_code.is_none() => {
                match process_result {
                    Ok(Ok(exit_code)) => {
                        process_exit_code = Some(exit_code);
                        drop(process_tree_guard.take());
                        output_drain_timeout
                            .as_mut()
                            .reset(output_drain_deadline()?);
                    }
                    Ok(Err(error)) => {
                        output_failure = Some(CommandError::Output(error.to_string()));
                        process_exit_code = Some(u32::MAX);
                        drop(process_tree_guard.take());
                        output_drain_timeout
                            .as_mut()
                            .reset(output_drain_deadline()?);
                    }
                    Err(error) => {
                        output_failure = Some(CommandError::Output(error.to_string()));
                        process_exit_code = Some(u32::MAX);
                        drop(process_tree_guard.take());
                        output_drain_timeout
                            .as_mut()
                            .reset(output_drain_deadline()?);
                    }
                }
            }
            termination_change = termination_receiver.changed(), if process_exit_code.is_none() && !termination_requested => {
                if termination_change.is_err() || *termination_receiver.borrow() {
                    terminated = true;
                    termination_requested = true;
                    drop(process_tree_guard.take());
                }
            }
            output_message = output_receiver.recv(), if closed_stream_count < 2 => {
                match output_message {
                    Some(OutputMessage::Chunk { destination, bytes }) => {
                        let destination_byte_count = match destination {
                            OutputDestination::StandardOutput => &mut standard_output_byte_count,
                            OutputDestination::StandardError => &mut standard_error_byte_count,
                        };

                        *destination_byte_count = destination_byte_count
                            .checked_add(bytes.len())
                            .ok_or(CommandError::OutputLimitExceeded)?;

                        combined_output_byte_count = combined_output_byte_count
                            .checked_add(bytes.len())
                            .ok_or(CommandError::OutputLimitExceeded)?;

                        let destination_limit = match destination {
                            OutputDestination::StandardOutput => MAXIMUM_STANDARD_OUTPUT_BYTES,
                            OutputDestination::StandardError => MAXIMUM_STANDARD_ERROR_BYTES,
                        };

                        if *destination_byte_count > destination_limit
                            || combined_output_byte_count > MAXIMUM_COMBINED_OUTPUT_BYTES
                        {
                            terminated = true;
                            output_failure = Some(CommandError::OutputLimitExceeded);
                            drop(process_tree_guard.take());
                        } else {
                            append_output(&terminal_logs, destination, &bytes)?;
                        }
                    }
                    Some(OutputMessage::Closed) | None => {
                        closed_stream_count = closed_stream_count.saturating_add(1);
                    }
                    Some(OutputMessage::Failed(error)) => {
                        output_failure = Some(CommandError::Output(error));
                        terminated = true;
                        drop(process_tree_guard.take());
                        closed_stream_count = closed_stream_count.saturating_add(1);
                    }
                }
            }
            _ = &mut output_drain_timeout, if process_exit_code.is_some() && closed_stream_count < 2 => {
                standard_output_reader.abort();
                standard_error_reader.abort();
                closed_stream_count = 2;
                output_failure = Some(CommandError::Output(
                    "output pipes remained open after process-tree termination".into(),
                ));
            }
        }
    }

    join_output_reader(standard_output_reader).await?;
    join_output_reader(standard_error_reader).await?;
    drop(concurrency_permit);

    workspace_access.revoke()?;

    if let Some(output_failure) = output_failure {
        return Err(output_failure);
    }

    let process_exit_code = process_exit_code
        .ok_or_else(|| CommandError::Output("process status was unavailable".into()))?;

    let exit_code = i32::from_ne_bytes(process_exit_code.to_ne_bytes());

    let final_terminal_logs = terminal_logs.lock().map_or_else(
        |poisoned| poisoned.into_inner().clone(),
        |terminal_logs| terminal_logs.clone(),
    );

    Ok(CommandOutcome {
        command: command_text,
        exit_code: Some(exit_code),
        succeeded: process_exit_code == 0,
        terminated,
        terminal_logs: final_terminal_logs,
    })
}
