use super::*;

#[cfg(unix)]
pub(super) fn configure_process_group(process_command: &mut Command) {
    process_command.process_group(0);
}

#[cfg(all(not(unix), not(windows)))]
pub(super) fn configure_process_group(_process_command: &mut Command) {}

#[cfg(unix)]
pub(super) async fn terminate_process_tree(
    process_identifier: Option<u32>,
    child_process: &mut Child,
) -> Result<(), CommandError> {
    terminate_remaining_descendants(process_identifier)?;

    match child_process.start_kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(CommandError::Termination(error.to_string())),
    }
}

#[cfg(unix)]
pub(super) fn terminate_remaining_descendants(
    process_identifier: Option<u32>,
) -> Result<(), CommandError> {
    let Some(process_identifier) = process_identifier else {
        return Ok(());
    };

    let process_group_identifier = i32::try_from(process_identifier)
        .map_err(|_| CommandError::Termination("process identifier is too large".into()))?;

    match nix::sys::signal::killpg(
        nix::unistd::Pid::from_raw(process_group_identifier),
        nix::sys::signal::Signal::SIGKILL,
    ) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(CommandError::Termination(error.to_string())),
    }
}

#[cfg(all(not(unix), not(windows)))]
pub(super) async fn terminate_process_tree(
    _process_identifier: Option<u32>,
    child_process: &mut Child,
) -> Result<(), CommandError> {
    child_process
        .kill()
        .await
        .map_err(|error| CommandError::Termination(error.to_string()))
}

#[cfg(all(not(unix), not(windows)))]
pub(super) fn terminate_remaining_descendants(
    _process_identifier: Option<u32>,
) -> Result<(), CommandError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn trusted_executable_directories() -> Vec<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
pub(super) fn trusted_executable_directories() -> Vec<PathBuf> {
    std::env::var_os("SystemRoot").map_or_else(Vec::new, |system_root| {
        vec![PathBuf::from(system_root).join("System32")]
    })
}

pub(super) fn default_system_directories() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        return ["/usr", "/bin", "/lib", "/lib64", "/etc"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
    }

    #[cfg(target_os = "macos")]
    {
        return ["/System", "/usr", "/bin", "/sbin"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
    }

    #[cfg(windows)]
    {
        return trusted_executable_directories();
    }

    #[allow(unreachable_code)]
    Vec::new()
}
