use super::*;

pub fn parse_posix_command(command: impl AsRef<str>) -> Result<CommandInvocation, CommandError> {
    let normalized_command = command.as_ref().trim();
    let command_parts = shlex::split(normalized_command).ok_or(CommandError::InvalidCommand)?;
    let Some((executable, arguments)) = command_parts.split_first() else {
        return Err(CommandError::EmptyCommand);
    };

    Ok(CommandInvocation {
        executable: OsString::from(executable),
        arguments: arguments.iter().map(OsString::from).collect(),
    })
}

pub fn execute_command(command: impl AsRef<str>) -> Result<CommandEntity, CommandError> {
    execute_command_in_directory(".", command)
}

pub fn execute_command_in_directory(
    working_directory: impl AsRef<Path>,
    command: impl AsRef<str>,
) -> Result<CommandEntity, CommandError> {
    let command_invocation = parse_posix_command(command)?;

    execute_command_in_directory_with_arguments(
        working_directory,
        command_invocation.executable,
        command_invocation.arguments,
    )
}

pub fn execute_command_in_directory_with_arguments<Arguments, Argument>(
    working_directory: impl AsRef<Path>,
    executable: impl Into<OsString>,
    arguments: Arguments,
) -> Result<CommandEntity, CommandError>
where
    Arguments: IntoIterator<Item = Argument>,
    Argument: Into<OsString>,
{
    execute_command_in_directory_with_arguments_and_options(
        working_directory,
        executable,
        arguments,
        SandboxedCommandOptions::default(),
    )
}

pub fn execute_command_in_directory_with_arguments_and_options<Arguments, Argument>(
    working_directory: impl AsRef<Path>,
    executable: impl Into<OsString>,
    arguments: Arguments,
    options: SandboxedCommandOptions,
) -> Result<CommandEntity, CommandError>
where
    Arguments: IntoIterator<Item = Argument>,
    Argument: Into<OsString>,
{
    let command_invocation = CommandInvocation {
        executable: executable.into(),
        arguments: arguments.into_iter().map(Into::into).collect(),
    };

    let authorization =
        prepare_sandboxed_command_with_options(working_directory, command_invocation, BTreeMap::new(), options)?;

    execute_sandboxed_command(authorization)
}

pub fn prepare_sandboxed_command(
    working_directory: impl AsRef<Path>,
    command_invocation: CommandInvocation,
    requested_environment: BTreeMap<OsString, OsString>,
    network_policy: SandboxNetworkPolicy,
) -> Result<SandboxedCommandAuthorization, CommandError> {
    prepare_sandboxed_command_with_options(
        working_directory,
        command_invocation,
        requested_environment,
        SandboxedCommandOptions::default().network_policy(network_policy),
    )
}

pub fn prepare_sandboxed_command_with_options(
    working_directory: impl AsRef<Path>,
    command_invocation: CommandInvocation,
    mut requested_environment: BTreeMap<OsString, OsString>,
    options: SandboxedCommandOptions,
) -> Result<SandboxedCommandAuthorization, CommandError> {
    if command_invocation.executable.is_empty() {
        return Err(CommandError::EmptyCommand);
    }

    ensure_sandbox_backend_available()?;

    let workspace_directory = std::fs::canonicalize(working_directory)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    let workspace_metadata = std::fs::metadata(&workspace_directory)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    if !workspace_metadata.is_dir() {
        return Err(CommandError::InvalidAuthorization(
            "working directory is not a directory".into(),
        ));
    }

    let executable_search_directories =
        configured_directories(&options.executable_search_directories, "executable search directory")?;

    let additional_read_only_directories = configured_directories(
        &options.additional_read_only_directories,
        "additional read-only directory",
    )?;

    requested_environment.extend(options.environment_variables);

    let mut trusted_search_directories = trusted_executable_directories();

    trusted_search_directories.extend(executable_search_directories.iter().cloned());
    deduplicate_paths(&mut trusted_search_directories);

    let executable_path = resolve_trusted_executable(&command_invocation.executable, &trusted_search_directories)?;

    let executable_metadata =
        std::fs::metadata(&executable_path).map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    if !executable_metadata.is_file() {
        return Err(CommandError::InvalidAuthorization(
            "executable path is not a regular file".into(),
        ));
    }

    let environment = build_sanitized_environment(requested_environment, &trusted_search_directories);

    Ok(SandboxedCommandAuthorization {
        executable_identity: executable_identity(&executable_path, &executable_metadata)?,
        executable_path,
        arguments: command_invocation.arguments,
        working_directory_identity: directory_identity(&workspace_directory, &workspace_metadata)?,
        environment,
        filesystem_policy: SandboxFilesystemPolicy {
            workspace_directory,
            read_only_system_directories: default_system_directories(),
            executable_search_directories,
            additional_read_only_directories,
        },
        network_policy: options.network_policy,
    })
}

fn configured_directories(directories: &[PathBuf], description: &str) -> Result<Vec<PathBuf>, CommandError> {
    let mut canonical_directories = Vec::with_capacity(directories.len());

    for directory in directories {
        if !directory.is_absolute() {
            return Err(CommandError::InvalidAuthorization(format!(
                "{description} must be absolute"
            )));
        }

        let canonical_directory =
            std::fs::canonicalize(directory).map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

        let metadata = std::fs::metadata(&canonical_directory)
            .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

        if !metadata.is_dir() {
            return Err(CommandError::InvalidAuthorization(format!(
                "{description} is not a directory"
            )));
        }

        canonical_directories.push(canonical_directory);
    }

    deduplicate_paths(&mut canonical_directories);

    Ok(canonical_directories)
}

fn deduplicate_paths(paths: &mut Vec<PathBuf>) {
    let mut unique_paths = Vec::with_capacity(paths.len());

    for path in paths.drain(..) {
        if !unique_paths.contains(&path) {
            unique_paths.push(path);
        }
    }

    *paths = unique_paths;
}
