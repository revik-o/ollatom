use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

use std::path::Path;

pub(super) fn validate_database_file_path(database_file_path: &Path) -> InfrastructureResult<()> {
    if database_file_path.as_os_str().is_empty() || !database_file_path.is_absolute() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidDatabaseFilePath,
            format!(
                "SQLite database path '{}' must be an absolute file path",
                database_file_path.display()
            ),
        ));
    }

    if database_file_path.is_dir() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabasePathPointsToDirectory,
            format!(
                "SQLite database path '{}' points to a directory",
                database_file_path.display()
            ),
        ));
    }

    let parent_directory_path = database_file_path.parent().ok_or_else(|| {
        InfrastructureError::new(
            InfrastructureErrorKind::InvalidDatabaseFilePath,
            format!(
                "SQLite database path '{}' has no parent directory",
                database_file_path.display()
            ),
        )
    })?;

    if !parent_directory_path.is_dir() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseParentDirectoryDoesNotExist,
            format!(
                "SQLite database parent directory '{}' does not exist",
                parent_directory_path.display()
            ),
        ));
    }

    Ok(())
}

pub(super) fn prepare_private_database_location(database_file_path: &Path) -> InfrastructureResult<()> {
    let database_directory_path = database_file_path.parent().ok_or_else(|| {
        InfrastructureError::new(
            InfrastructureErrorKind::InvalidDatabaseFilePath,
            "SQLite database path has no parent directory",
        )
    })?;
    filesystem::restrict_directory_to_current_user(database_directory_path).map_err(|source| {
        InfrastructureError::from_database_error(
            InfrastructureErrorKind::DatabaseConfigurationFailed,
            "failed to restrict SQLite database directory access",
            source,
        )
    })?;

    let mut database_file = open_or_create_private_database_file(database_file_path)?;
    filesystem::restrict_open_file_to_current_user(&mut database_file).map_err(|source| {
        InfrastructureError::from_database_error(
            InfrastructureErrorKind::DatabaseConfigurationFailed,
            "failed to restrict SQLite database file access",
            source,
        )
    })?;

    Ok(())
}

#[cfg(unix)]
fn open_or_create_private_database_file(database_file_path: &Path) -> InfrastructureResult<std::fs::File> {
    let mut create_options = std::fs::OpenOptions::new();
    create_options
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits());

    match create_options.open(database_file_path) {
        Ok(database_file) => Ok(database_file),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut open_options = std::fs::OpenOptions::new();

            open_options
                .read(true)
                .write(true)
                .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits());

            open_options.open(database_file_path).map_err(|source| {
                InfrastructureError::from_database_error(
                    InfrastructureErrorKind::DatabaseConfigurationFailed,
                    "failed to open existing SQLite database without following symlinks",
                    source,
                )
            })
        }
        Err(source) => Err(InfrastructureError::from_database_error(
            InfrastructureErrorKind::DatabaseConfigurationFailed,
            "failed to create SQLite database without following symlinks",
            source,
        )),
    }
}

#[cfg(windows)]
fn open_or_create_private_database_file(database_file_path: &Path) -> InfrastructureResult<std::fs::File> {
    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
    const FILE_READ_CONTROL: u32 = 0x0002_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const WRITE_DAC: u32 = 0x0004_0000;

    let mut create_options = std::fs::OpenOptions::new();

    create_options
        .read(true)
        .write(true)
        .create_new(true)
        .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_READ_CONTROL | WRITE_DAC)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

    match create_options.open(database_file_path) {
        Ok(database_file) => Ok(database_file),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut open_options = std::fs::OpenOptions::new();

            open_options
                .read(true)
                .write(true)
                .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_READ_CONTROL | WRITE_DAC)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

            open_options.open(database_file_path).map_err(|source| {
                InfrastructureError::from_database_error(
                    InfrastructureErrorKind::DatabaseConfigurationFailed,
                    "failed to open existing SQLite database without following reparse points",
                    source,
                )
            })
        }
        Err(source) => Err(InfrastructureError::from_database_error(
            InfrastructureErrorKind::DatabaseConfigurationFailed,
            "failed to create SQLite database without following reparse points",
            source,
        )),
    }
}

#[cfg(not(any(unix, windows)))]
fn open_or_create_private_database_file(database_file_path: &Path) -> InfrastructureResult<std::fs::File> {
    Err(InfrastructureError::new(
        InfrastructureErrorKind::DatabaseConfigurationFailed,
        format!(
            "owner-only SQLite permissions are unavailable for '{}'",
            database_file_path.display()
        ),
    ))
}
