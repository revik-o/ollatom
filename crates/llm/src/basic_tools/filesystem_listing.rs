use super::BasicToolConfiguration;
use crate::ToolFailure;
use serde_json::{Value, json};
use std::{fs, io, path::Path, sync::Arc};

pub(super) async fn list_files(
    configuration: Arc<BasicToolConfiguration>,
) -> Result<Value, ToolFailure> {
    tokio::task::spawn_blocking(move || collect_relative_file_paths(&configuration))
        .await
        .map_err(|error| ToolFailure::Execution(error.to_string()))?
}

fn collect_relative_file_paths(
    configuration: &BasicToolConfiguration,
) -> Result<Value, ToolFailure> {
    let mut relative_file_paths = Vec::new();

    collect_relative_file_paths_from_directory(
        configuration.root_directory(),
        configuration.root_directory(),
        configuration,
        &mut relative_file_paths,
    )
    .map_err(input_output_failure)?;

    relative_file_paths.sort();

    Ok(json!({"tool": "list_files", "files": relative_file_paths}))
}

fn collect_relative_file_paths_from_directory(
    root_directory: &Path,
    current_directory: &Path,
    configuration: &BasicToolConfiguration,
    relative_file_paths: &mut Vec<String>,
) -> io::Result<()> {
    for directory_entry in fs::read_dir(current_directory)? {
        let directory_entry = directory_entry?;
        let entry_path = directory_entry.path();
        let entry_metadata = fs::symlink_metadata(&entry_path)?;

        if entry_metadata.file_type().is_symlink() {
            continue;
        }

        if entry_metadata.is_dir() {
            let directory_name = directory_entry.file_name().to_string_lossy().into_owned();

            if configuration
                .excluded_directory_names()
                .contains(&directory_name)
            {
                continue;
            }

            collect_relative_file_paths_from_directory(
                root_directory,
                &entry_path,
                configuration,
                relative_file_paths,
            )?;
        }

        if entry_metadata.is_file() {
            relative_file_paths.push(
                entry_path
                    .strip_prefix(root_directory)
                    .map_err(io::Error::other)?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }

    Ok(())
}

fn input_output_failure(error: io::Error) -> ToolFailure {
    ToolFailure::Execution(error.to_string())
}
