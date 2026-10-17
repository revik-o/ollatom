use super::*;

#[cfg(target_os = "linux")]
pub(super) fn ensure_sandbox_backend_available() -> Result<(), CommandError> {
    let backend_path = Path::new("/usr/bin/bwrap");

    backend_path
        .is_file()
        .then_some(())
        .ok_or_else(|| CommandError::SandboxUnavailable("/usr/bin/bwrap is required".into()))
}

#[cfg(target_os = "macos")]
pub(super) fn ensure_sandbox_backend_available() -> Result<(), CommandError> {
    let backend_path = Path::new("/usr/bin/sandbox-exec");

    backend_path
        .is_file()
        .then_some(())
        .ok_or_else(|| CommandError::SandboxUnavailable("/usr/bin/sandbox-exec is required".into()))
}

#[cfg(windows)]
pub(super) fn ensure_sandbox_backend_available() -> Result<(), CommandError> {
    command_windows::ensure_sandbox_backend_available()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) fn ensure_sandbox_backend_available() -> Result<(), CommandError> {
    Err(CommandError::SandboxUnavailable(
        "this operating system has no supported sandbox backend".into(),
    ))
}

#[cfg(target_os = "linux")]
pub(super) fn build_sandbox_process(
    authorization: &SandboxedCommandAuthorization,
    pinned_executable: &std::fs::File,
) -> Result<Command, CommandError> {
    let mut command = Command::new("/usr/bin/bwrap");

    command
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-all")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--dir")
        .arg("/tmp/home");

    if authorization.network_policy == SandboxNetworkPolicy::Allowed {
        command.arg("--share-net");
    }

    for system_directory in &authorization.filesystem_policy.read_only_system_directories {
        if system_directory.exists() {
            command.arg("--ro-bind").arg(system_directory).arg(system_directory);
        }
    }

    for additional_directory in &authorization.filesystem_policy.additional_read_only_directories {
        let mut parent_directories = additional_directory
            .ancestors()
            .filter(|directory| *directory != Path::new("/"))
            .collect::<Vec<_>>();

        parent_directories.reverse();

        for parent_directory in parent_directories {
            command.arg("--dir").arg(parent_directory);
        }

        command
            .arg("--ro-bind")
            .arg(additional_directory)
            .arg(additional_directory);
    }

    command
        .arg("--bind")
        .arg(&authorization.filesystem_policy.workspace_directory)
        .arg(&authorization.filesystem_policy.workspace_directory)
        .arg("--chdir")
        .arg(&authorization.filesystem_policy.workspace_directory)
        .arg("--clearenv");

    for (name, value) in &authorization.environment {
        command.arg("--setenv").arg(name).arg(value);
    }

    command.arg("--ro-bind-fd").arg("0").arg(&authorization.executable_path);

    let pinned_executable_for_child = pinned_executable
        .try_clone()
        .map_err(|error| CommandError::Spawn(error.to_string()))?;

    command
        .arg("--")
        .arg("/bin/bash")
        .arg("-c")
        .arg("exec 0</dev/null; exec -a \"$0\" \"$0\" \"$@\"")
        .arg(&authorization.executable_path)
        .args(&authorization.arguments);

    command.stdin(Stdio::from(pinned_executable_for_child));

    Ok(command)
}

#[cfg(target_os = "macos")]
pub(super) fn build_sandbox_process(
    authorization: &SandboxedCommandAuthorization,
    pinned_executable: &std::fs::File,
) -> Result<Command, CommandError> {
    let escaped_workspace = authorization
        .filesystem_policy
        .workspace_directory
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");

    let mut sandbox_policy = format!(
        "(version 1)(deny default)(allow process*)(allow file-read* (subpath \"/System\") (subpath \"/usr\") (subpath \"/bin\") (subpath \"/sbin\"))(allow file-read* file-write* (subpath \"{escaped_workspace}\"))(allow file-read* file-write* (subpath \"/private/tmp\"))"
    );

    for additional_directory in &authorization.filesystem_policy.additional_read_only_directories {
        let escaped_directory = additional_directory
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        sandbox_policy.push_str(&format!("(allow file-read* (subpath \"{escaped_directory}\"))"));
    }

    if authorization.network_policy == SandboxNetworkPolicy::Allowed {
        sandbox_policy.push_str("(allow network*)");
    }

    let mut command = Command::new("/usr/bin/sandbox-exec");

    command
        .arg("-p")
        .arg(sandbox_policy)
        .arg("/bin/bash")
        .arg("-c")
        .arg("exec 3<&0; exec 0</dev/null; exec -a \"$0\" /dev/fd/3 \"$@\"")
        .arg(&authorization.executable_path)
        .args(&authorization.arguments)
        .current_dir(&authorization.filesystem_policy.workspace_directory)
        .env_clear()
        .envs(&authorization.environment);

    let pinned_executable_for_child = pinned_executable
        .try_clone()
        .map_err(|error| CommandError::Spawn(error.to_string()))?;

    command.stdin(Stdio::from(pinned_executable_for_child));

    Ok(command)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(super) fn build_sandbox_process(
    _authorization: &SandboxedCommandAuthorization,
    _pinned_executable: &std::fs::File,
) -> Result<Command, CommandError> {
    Err(CommandError::SandboxUnavailable(
        "this operating system has no supported sandbox backend".into(),
    ))
}
