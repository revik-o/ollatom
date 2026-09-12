use crate::{BenchmarkArtifacts, StageLogger};
use os::{CommandOutcome, execute_command_in_directory_with_arguments};
use serde_json::json;
use std::{fs, io, time::Duration};

const INDEPENDENT_BUILD_TIMEOUT: Duration = Duration::from_mins(15);
const TERMINATION_COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn verify_independent_build(
    artifacts: &BenchmarkArtifacts,
    logger: &StageLogger,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    artifacts.remove_stale_dist()?;
    let command_outcome = execute_build_command(artifacts).await?;

    logger.write_line(serde_json::to_string(&json!({
        "verification": "independent_build",
        "command": ["npm", "run", "build"],
        "success": command_outcome.succeeded,
        "exit_code": command_outcome.exit_code,
        "stdout": command_outcome.terminal_logs.standard_output,
        "stderr": command_outcome.terminal_logs.standard_error
    }))?)?;

    if !command_outcome.succeeded {
        return Err(format!(
            "independent npm build failed: {}",
            command_outcome.exit_code.unwrap_or(-1)
        )
        .into());
    }

    validate_distribution_output(artifacts)?;
    logger.write_line("independent build verification succeeded")?;

    Ok(())
}

async fn execute_build_command(artifacts: &BenchmarkArtifacts) -> io::Result<CommandOutcome> {
    let command_entity = execute_command_in_directory_with_arguments(
        artifacts.project_root(),
        "npm",
        ["run".to_owned(), "build".to_owned()],
    )
    .map_err(command_error)?;
    let mut command_future = Box::pin(command_entity);

    match tokio::time::timeout(INDEPENDENT_BUILD_TIMEOUT, &mut command_future).await {
        Ok(command_result) => command_result.map_err(command_error),
        Err(_) => {
            command_future
                .as_ref()
                .get_ref()
                .terminate()
                .map_err(command_error)?;
            let _ = tokio::time::timeout(TERMINATION_COMPLETION_TIMEOUT, command_future).await;
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "independent build timed out",
            ))
        }
    }
}

fn validate_distribution_output(
    artifacts: &BenchmarkArtifacts,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let distribution_directory_path = artifacts.project_root().join("dist");
    let distribution_directory_metadata = fs::symlink_metadata(&distribution_directory_path)?;

    if distribution_directory_metadata.file_type().is_symlink()
        || !distribution_directory_metadata.is_dir()
    {
        return Err("dist must be a regular directory".into());
    }

    let index_file_path = distribution_directory_path.join("index.html");
    let index_file_metadata = fs::symlink_metadata(&index_file_path)?;

    if index_file_metadata.file_type().is_symlink() || !index_file_metadata.is_file() {
        return Err("dist/index.html is missing or not a regular file".into());
    }

    let mut resource_count = 0usize;
    count_distribution_resources(
        &distribution_directory_path,
        &index_file_path,
        &mut resource_count,
    )?;

    if resource_count == 0 {
        return Err("dist does not contain a resource besides index.html".into());
    }

    Ok(())
}

fn count_distribution_resources(
    directory: &std::path::Path,
    index_file_path: &std::path::Path,
    resource_count: &mut usize,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;

        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "dist contains a symlink",
            ));
        }

        if metadata.is_dir() {
            count_distribution_resources(&path, index_file_path, resource_count)?;
        } else if metadata.is_file() && path != index_file_path {
            *resource_count += 1;
        }
    }

    Ok(())
}

fn command_error(error: os::CommandError) -> io::Error {
    io::Error::other(error.to_string())
}
