use super::*;

#[cfg(unix)]
pub(super) fn executable_identity(
    _path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<ExecutableIdentity, CommandError> {
    use std::os::unix::fs::MetadataExt;

    Ok(ExecutableIdentity::Unix {
        device_identifier: metadata.dev(),
        inode_identifier: metadata.ino(),
    })
}

#[cfg(unix)]
pub(super) fn directory_identity(_path: &Path, metadata: &std::fs::Metadata) -> Result<DirectoryIdentity, CommandError> {
    use std::os::unix::fs::MetadataExt;

    Ok(DirectoryIdentity::Unix {
        device_identifier: metadata.dev(),
        inode_identifier: metadata.ino(),
    })
}

#[cfg(windows)]
pub(super) fn executable_identity(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<ExecutableIdentity, CommandError> {
    if !metadata.is_file() {
        return Err(CommandError::InvalidAuthorization(
            "executable path is not a regular file".into(),
        ));
    }

    let (volume_identifier, file_identifier) = windows_file_identity(path)?;

    Ok(ExecutableIdentity::Windows {
        volume_identifier,
        file_identifier,
    })
}

#[cfg(windows)]
pub(super) fn directory_identity(path: &Path, metadata: &std::fs::Metadata) -> Result<DirectoryIdentity, CommandError> {
    if !metadata.is_dir() {
        return Err(CommandError::InvalidAuthorization(
            "working directory is not a directory".into(),
        ));
    }

    let (volume_identifier, file_identifier) = windows_file_identity(path)?;

    Ok(DirectoryIdentity::Windows {
        volume_identifier,
        file_identifier,
    })
}

#[cfg(windows)]
pub(super) fn windows_file_identity(path: &Path) -> Result<(Vec<u8>, [u8; 16]), CommandError> {
    match win_file_id::get_high_res_file_id(path)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?
    {
        win_file_id::FileId::HighRes {
            volume_serial_number,
            file_id,
        } => Ok((volume_serial_number.to_le_bytes().to_vec(), file_id.to_le_bytes())),
        win_file_id::FileId::LowRes { .. } => Err(CommandError::InvalidAuthorization(
            "high-resolution Windows file identity is unavailable".into(),
        )),
    }
}

pub(super) fn display_command(executable: &Path, arguments: &[OsString]) -> String {
    std::iter::once(executable.as_os_str())
        .chain(arguments.iter().map(OsString::as_os_str))
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}
