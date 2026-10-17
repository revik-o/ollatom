use super::BasicToolConfiguration;
use crate::ToolFailure;
use serde_json::{Value, json};
use std::{fs, io, path::PathBuf, sync::Arc};

const MAXIMUM_FILE_COUNT: usize = 10_000;
const MAXIMUM_SERIALIZED_BYTE_COUNT: usize = 4 * 1024 * 1024;

pub(super) async fn list_files(
    configuration: Arc<BasicToolConfiguration>,
    stop: crate::StopToken,
) -> Result<Value, ToolFailure> {
    let worker_stop = stop.clone();
    let mut worker = tokio::task::spawn_blocking(move || collect_relative_file_paths(&configuration, &worker_stop));

    tokio::select! {
        result = &mut worker => result.map_err(|error| ToolFailure::Execution(error.to_string()))?,
        () = stop.cancelled() => {
            worker.await.map_err(|error| ToolFailure::Execution(error.to_string()))??;
            Err(ToolFailure::Timeout("file listing cancelled".into()))
        }
    }
}

fn collect_relative_file_paths(
    configuration: &BasicToolConfiguration,
    stop: &crate::StopToken,
) -> Result<Value, ToolFailure> {
    let mut relative_file_paths = Vec::new();
    let mut pending_directories = vec![configuration.root_directory().to_owned()];
    let mut serialized_byte_count = 0usize;

    while let Some(current_directory) = pending_directories.pop() {
        if stop.is_stopped() {
            return Err(ToolFailure::Timeout("file listing cancelled".into()));
        }

        collect_directory_entries(
            configuration,
            current_directory,
            stop,
            &mut pending_directories,
            &mut relative_file_paths,
            &mut serialized_byte_count,
        )
        .map_err(input_output_failure)?;
    }

    relative_file_paths.sort();

    Ok(json!({"tool": "list_files", "files": relative_file_paths}))
}

fn collect_directory_entries(
    configuration: &BasicToolConfiguration,
    current_directory: PathBuf,
    stop: &crate::StopToken,
    pending_directories: &mut Vec<PathBuf>,
    relative_file_paths: &mut Vec<String>,
    serialized_byte_count: &mut usize,
) -> io::Result<()> {
    for directory_entry in fs::read_dir(current_directory)? {
        if stop.is_stopped() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "file listing cancelled"));
        }

        let directory_entry = directory_entry?;
        let entry_path = directory_entry.path();
        let entry_metadata = fs::symlink_metadata(&entry_path)?;

        if entry_metadata.file_type().is_symlink() {
            continue;
        }

        if entry_metadata.is_dir() {
            let directory_name = directory_entry.file_name().to_string_lossy().into_owned();

            if configuration.excluded_directory_names().contains(&directory_name) {
                continue;
            }

            pending_directories.push(entry_path.clone());
        }

        if entry_metadata.is_file() {
            let relative_path = entry_path
                .strip_prefix(configuration.root_directory())
                .map_err(io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");

            *serialized_byte_count = serialized_byte_count
                .checked_add(
                    relative_path
                        .len()
                        .checked_add(3)
                        .ok_or_else(|| io::Error::other("file listing size limit exceeded"))?,
                )
                .ok_or_else(|| io::Error::other("file listing size limit exceeded"))?;

            if relative_file_paths.len() >= MAXIMUM_FILE_COUNT || *serialized_byte_count > MAXIMUM_SERIALIZED_BYTE_COUNT
            {
                return Err(io::Error::other("file listing limit exceeded"));
            }

            relative_file_paths.push(relative_path);
        }
    }

    Ok(())
}

fn input_output_failure(error: io::Error) -> ToolFailure {
    ToolFailure::Execution(error.to_string())
}
