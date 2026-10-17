use super::*;

pub(super) async fn monitor_command(
    command_text: String,
    authorization: SandboxedCommandAuthorization,
    terminal_logs: Arc<Mutex<TerminalLogs>>,
    termination_receiver: watch::Receiver<bool>,
) -> Result<CommandOutcome, CommandError> {
    #[cfg(windows)]
    {
        command_windows::monitor_command(command_text, authorization, terminal_logs, termination_receiver).await
    }

    #[cfg(not(windows))]
    {
        let mut termination_receiver = termination_receiver;

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

        let pinned_workspace = open_authorized_workspace(&authorization)?;
        let pinned_executable = open_authorized_executable(&authorization)?;
        let mut process_command = build_sandbox_process(&authorization, &pinned_executable)?;

        process_command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure_process_group(&mut process_command);
        validate_pinned_workspace_path(&authorization, &pinned_workspace)?;
        validate_pinned_executable_path(&authorization, &pinned_executable)?;

        let mut child_process = process_command
            .spawn()
            .map_err(|error| CommandError::Spawn(error.to_string()))?;

        let process_identifier = child_process.id();
        let standard_output = child_process
            .stdout
            .take()
            .ok_or_else(|| CommandError::Output("standard output pipe was unavailable".into()))?;

        let standard_error = child_process
            .stderr
            .take()
            .ok_or_else(|| CommandError::Output("standard error pipe was unavailable".into()))?;

        let (output_sender, mut output_receiver) = mpsc::channel(64);

        let standard_output_reader = spawn_output_reader(
            standard_output,
            OutputDestination::StandardOutput,
            output_sender.clone(),
        );

        let standard_error_reader = spawn_output_reader(standard_error, OutputDestination::StandardError, output_sender);

        let mut closed_stream_count = 0_usize;
        let mut standard_output_byte_count = 0_usize;
        let mut standard_error_byte_count = 0_usize;
        let mut combined_output_byte_count = 0_usize;
        let mut process_status = None;
        let mut terminated = false;
        let mut output_failure = None;

        let output_drain_timeout = tokio::time::sleep(OUTPUT_DRAIN_TIMEOUT);

        tokio::pin!(output_drain_timeout);

        while process_status.is_none() || closed_stream_count < 2 {
            tokio::select! {
                biased;
                termination_change = termination_receiver.changed(), if process_status.is_none() => {
                    if termination_change.is_err() || *termination_receiver.borrow() {
                        terminated = true;
                        terminate_process_tree(process_identifier, &mut child_process).await?;
                        process_status = Some(
                            child_process
                                .wait()
                                .await
                                .map_err(|error| CommandError::Termination(error.to_string()))?
                        );
                        output_drain_timeout
                            .as_mut()
                            .reset(output_drain_deadline()?);
                    }
                }
                status = child_process.wait(), if process_status.is_none() => {
                    process_status = Some(
                        status.map_err(|error| CommandError::Output(error.to_string()))?
                    );
                    terminate_remaining_descendants(process_identifier)?;
                    output_drain_timeout
                        .as_mut()
                        .reset(output_drain_deadline()?);
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
                                terminate_process_tree(process_identifier, &mut child_process).await?;

                                if process_status.is_none() {
                                    process_status = Some(
                                        child_process
                                            .wait()
                                            .await
                                            .map_err(|error| CommandError::Termination(error.to_string()))?
                                    );
                                    output_drain_timeout
                                        .as_mut()
                                        .reset(output_drain_deadline()?);
                                }

                                output_failure = Some(CommandError::OutputLimitExceeded);
                            } else {
                                append_output(&terminal_logs, destination, &bytes)?;
                            }
                        }
                        Some(OutputMessage::Closed) | None => {
                            closed_stream_count = closed_stream_count.saturating_add(1);
                        }
                        Some(OutputMessage::Failed(error)) => {
                            output_failure = Some(CommandError::Output(error));
                            closed_stream_count = closed_stream_count.saturating_add(1);
                        }
                    }
                }

                _ = &mut output_drain_timeout, if process_status.is_some() && closed_stream_count < 2 => {
                    terminate_remaining_descendants(process_identifier)?;
                    standard_output_reader.abort();
                    standard_error_reader.abort();
                    closed_stream_count = 2;
                    output_failure = Some(CommandError::Output(
                        "output pipes remained open after descendant termination".into(),
                    ));
                }
            }
        }

        join_output_reader(standard_output_reader).await?;
        join_output_reader(standard_error_reader).await?;
        drop(concurrency_permit);

        if let Some(output_failure) = output_failure {
            return Err(output_failure);
        }

        let process_status =
            process_status.ok_or_else(|| CommandError::Output("process status was unavailable".into()))?;

        let final_terminal_logs = terminal_logs.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |terminal_logs| terminal_logs.clone(),
        );

        Ok(CommandOutcome {
            command: command_text,
            exit_code: process_status.code(),
            succeeded: process_status.success(),
            terminated,
            terminal_logs: final_terminal_logs,
        })
    }
}
