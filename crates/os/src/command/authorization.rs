use super::*;

pub(super) fn validate_authorization_identity(
    authorization: &SandboxedCommandAuthorization,
) -> Result<(), CommandError> {
    let _pinned_workspace = open_authorized_workspace(authorization)?;
    let _pinned_executable = open_authorized_executable(authorization)?;

    Ok(())
}

pub(super) fn open_authorized_executable(
    authorization: &SandboxedCommandAuthorization,
) -> Result<std::fs::File, CommandError> {
    let pinned_executable = open_executable_without_replacement(&authorization.executable_path)?;

    validate_pinned_executable_path(authorization, &pinned_executable)?;

    Ok(pinned_executable)
}

pub(super) fn open_authorized_workspace(
    authorization: &SandboxedCommandAuthorization,
) -> Result<std::fs::File, CommandError> {
    let pinned_workspace = open_directory_without_replacement(&authorization.filesystem_policy.workspace_directory)?;

    validate_pinned_workspace_path(authorization, &pinned_workspace)?;

    Ok(pinned_workspace)
}

pub(super) fn validate_pinned_executable_path(
    authorization: &SandboxedCommandAuthorization,
    pinned_executable: &std::fs::File,
) -> Result<(), CommandError> {
    let pinned_metadata = pinned_executable
        .metadata()
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    let executable_path_metadata = std::fs::symlink_metadata(&authorization.executable_path)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    if executable_path_metadata.file_type().is_symlink()
        || !executable_path_metadata.is_file()
        || !pinned_metadata.is_file()
        || executable_identity(&authorization.executable_path, &pinned_metadata)? != authorization.executable_identity
        || executable_identity(&authorization.executable_path, &executable_path_metadata)?
            != authorization.executable_identity
    {
        return Err(CommandError::InvalidAuthorization(
            "approved executable identity changed".into(),
        ));
    }

    Ok(())
}

pub(super) fn validate_pinned_workspace_path(
    authorization: &SandboxedCommandAuthorization,
    pinned_workspace: &std::fs::File,
) -> Result<(), CommandError> {
    let pinned_metadata = pinned_workspace
        .metadata()
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    let workspace_path_metadata = std::fs::symlink_metadata(&authorization.filesystem_policy.workspace_directory)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    if workspace_path_metadata.file_type().is_symlink()
        || !workspace_path_metadata.is_dir()
        || !pinned_metadata.is_dir()
        || directory_identity(&authorization.filesystem_policy.workspace_directory, &pinned_metadata)?
            != authorization.working_directory_identity
        || directory_identity(
            &authorization.filesystem_policy.workspace_directory,
            &workspace_path_metadata,
        )? != authorization.working_directory_identity
    {
        return Err(CommandError::InvalidAuthorization(
            "approved working directory identity changed".into(),
        ));
    }

    Ok(())
}

pub(super) fn open_executable_without_replacement(path: &Path) -> Result<std::fs::File, CommandError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ_ONLY)
            .open(path)
            .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))
    }

    #[cfg(not(windows))]
    {
        std::fs::File::open(path).map_err(|error| CommandError::InvalidAuthorization(error.to_string()))
    }
}

pub(super) fn open_directory_without_replacement(path: &Path) -> Result<std::fs::File, CommandError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        std::fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))
    }

    #[cfg(not(windows))]
    {
        std::fs::File::open(path).map_err(|error| CommandError::InvalidAuthorization(error.to_string()))
    }
}

#[cfg(windows)]
const FILE_SHARE_READ_ONLY: u32 = 0x0000_0001;

#[cfg(windows)]
const FILE_SHARE_READ_WRITE: u32 = 0x0000_0003;

#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

pub(super) fn resolve_trusted_executable(
    executable: &OsStr,
    trusted_search_directories: &[PathBuf],
) -> Result<PathBuf, CommandError> {
    let executable_path = Path::new(executable);

    if executable_path.components().count() > 1 || executable_path.is_absolute() {
        return std::fs::canonicalize(executable_path)
            .map_err(|error| CommandError::InvalidAuthorization(error.to_string()));
    }

    trusted_search_directories
        .iter()
        .cloned()
        .into_iter()
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| std::fs::canonicalize(candidate).ok())
        .ok_or_else(|| CommandError::InvalidAuthorization("executable was not found in the trusted PATH".into()))
}
