use super::command::CommandError;
use std::{
    io::Read,
    sync::{Arc, Mutex},
    thread,
};

const OUTPUT_BUFFER_SIZE: usize = 8192;

#[derive(Clone, Copy)]
pub(super) enum TerminalOutputDestination {
    StandardOutput,
    StandardError,
}

pub(super) fn spawn_output_reader(
    stream: Option<impl Read + Send + 'static>,
    terminal_logs: Arc<Mutex<super::command::TerminalLogs>>,
    output_destination: TerminalOutputDestination,
) -> Option<thread::JoinHandle<Result<(), CommandError>>> {
    stream.map(|mut stream| {
        thread::spawn(move || {
            let mut buffer = [0_u8; OUTPUT_BUFFER_SIZE];
            let mut incomplete_utf8_bytes = Vec::new();

            loop {
                let bytes_read = stream
                    .read(&mut buffer)
                    .map_err(|error| CommandError::Output(error.to_string()))?;

                if bytes_read == 0 {
                    append_incomplete_utf8_bytes(
                        &terminal_logs,
                        output_destination,
                        &mut incomplete_utf8_bytes,
                    )?;
                    return Ok(());
                }

                append_terminal_output(
                    &terminal_logs,
                    output_destination,
                    &mut incomplete_utf8_bytes,
                    &buffer[..bytes_read],
                )?;
            }
        })
    })
}

fn append_terminal_output(
    terminal_logs: &Arc<Mutex<super::command::TerminalLogs>>,
    output_destination: TerminalOutputDestination,
    incomplete_utf8_bytes: &mut Vec<u8>,
    output_bytes: &[u8],
) -> Result<(), CommandError> {
    incomplete_utf8_bytes.extend_from_slice(output_bytes);
    let decoded_output = decode_complete_utf8_output(incomplete_utf8_bytes);
    append_text_to_terminal_logs(terminal_logs, output_destination, &decoded_output)
}

fn append_incomplete_utf8_bytes(
    terminal_logs: &Arc<Mutex<super::command::TerminalLogs>>,
    output_destination: TerminalOutputDestination,
    incomplete_utf8_bytes: &mut Vec<u8>,
) -> Result<(), CommandError> {
    let decoded_output = String::from_utf8_lossy(incomplete_utf8_bytes).into_owned();
    incomplete_utf8_bytes.clear();

    append_text_to_terminal_logs(terminal_logs, output_destination, &decoded_output)
}

fn decode_complete_utf8_output(incomplete_utf8_bytes: &mut Vec<u8>) -> String {
    let mut decoded_output = String::new();

    loop {
        match std::str::from_utf8(incomplete_utf8_bytes) {
            Ok(valid_output) => {
                decoded_output.push_str(valid_output);
                incomplete_utf8_bytes.clear();
                break;
            }
            Err(validation_error) => {
                let valid_byte_count = validation_error.valid_up_to();
                decoded_output.push_str(&String::from_utf8_lossy(
                    &incomplete_utf8_bytes[..valid_byte_count],
                ));
                incomplete_utf8_bytes.drain(..valid_byte_count);

                let Some(invalid_byte_count) = validation_error.error_len() else {
                    break;
                };
                decoded_output.push('\u{FFFD}');
                incomplete_utf8_bytes.drain(..invalid_byte_count);
            }
        }
    }

    decoded_output
}

fn append_text_to_terminal_logs(
    terminal_logs: &Arc<Mutex<super::command::TerminalLogs>>,
    output_destination: TerminalOutputDestination,
    decoded_output: &str,
) -> Result<(), CommandError> {
    if decoded_output.is_empty() {
        return Ok(());
    }

    let mut terminal_logs = terminal_logs
        .lock()
        .map_err(|_| CommandError::Output("terminal log lock was poisoned".into()))?;

    match output_destination {
        TerminalOutputDestination::StandardOutput => {
            terminal_logs.standard_output.push_str(decoded_output);
        }
        TerminalOutputDestination::StandardError => {
            terminal_logs.standard_error.push_str(decoded_output);
        }
    }

    Ok(())
}
