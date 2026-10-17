use super::*;

pub(super) fn spawn_output_reader<OutputStream>(
    mut output_stream: OutputStream,
    destination: OutputDestination,
    output_sender: mpsc::Sender<OutputMessage>,
) -> JoinHandle<()>
where
    OutputStream: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut output_buffer = [0_u8; OUTPUT_BUFFER_BYTE_COUNT];

        loop {
            match output_stream.read(&mut output_buffer).await {
                Ok(0) => {
                    let _ = output_sender.send(OutputMessage::Closed).await;
                    return;
                }
                Ok(bytes_read) => {
                    if output_sender
                        .send(OutputMessage::Chunk {
                            destination,
                            bytes: output_buffer[..bytes_read].to_vec(),
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    let _ = output_sender.send(OutputMessage::Failed(error.to_string())).await;
                    return;
                }
            }
        }
    })
}

pub(super) async fn join_output_reader(output_reader: JoinHandle<()>) -> Result<(), CommandError> {
    match output_reader.await {
        Ok(()) => Ok(()),
        Err(error) if error.is_cancelled() => Ok(()),
        Err(error) => Err(CommandError::Output(error.to_string())),
    }
}

pub(super) fn append_output(
    terminal_logs: &Arc<Mutex<TerminalLogs>>,
    destination: OutputDestination,
    bytes: &[u8],
) -> Result<(), CommandError> {
    let decoded_output = String::from_utf8_lossy(bytes);
    let mut terminal_logs = terminal_logs
        .lock()
        .map_err(|_| CommandError::Output("terminal log lock was poisoned".into()))?;

    match destination {
        OutputDestination::StandardOutput => {
            terminal_logs.standard_output.push_str(&decoded_output);
        }
        OutputDestination::StandardError => {
            terminal_logs.standard_error.push_str(&decoded_output);
        }
    }
    terminal_logs.combined_output.push_str(&decoded_output);

    Ok(())
}

pub(super) fn command_semaphore() -> &'static Arc<Semaphore> {
    static COMMAND_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

    COMMAND_SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(MAXIMUM_CONCURRENT_COMMAND_COUNT)))
}

pub(super) fn output_drain_deadline() -> Result<tokio::time::Instant, CommandError> {
    tokio::time::Instant::now()
        .checked_add(OUTPUT_DRAIN_TIMEOUT)
        .ok_or_else(|| CommandError::Output("output drain deadline overflowed".into()))
}
