use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult, ProjectFilesystemIdentity};
use filesystem::{DirectoryIdentity, WorkspaceDirectory};
use std::path::{Path, PathBuf};

pub(super) async fn resolve_project_directory(
    project_path: &Path,
) -> InfrastructureResult<(PathBuf, String, ProjectFilesystemIdentity)> {
    let workspace_directory = WorkspaceDirectory::open(project_path)
        .await
        .map_err(|source| invalid_project_path(project_path, source.to_string()))?;

    let canonical_directory_path = workspace_directory.canonical_directory_path().to_owned();
    let canonical_path_text = match canonical_directory_path.to_str() {
        Some(path_text) => path_text.to_owned(),
        None => {
            return Err(invalid_project_path(
                &canonical_directory_path,
                "canonical directory paths must be valid UTF-8",
            ));
        }
    };

    let filesystem_identity = match workspace_directory.filesystem_identity() {
        #[cfg(unix)]
        DirectoryIdentity::Unix {
            device_identifier,
            inode_identifier,
        } => ProjectFilesystemIdentity::Unix {
            device_identifier: *device_identifier,
            inode_identifier: *inode_identifier,
        },
        #[cfg(windows)]
        DirectoryIdentity::Windows {
            volume_identifier,
            file_identifier,
        } => ProjectFilesystemIdentity::Windows {
            volume_identifier: volume_identifier.clone(),
            file_identifier: *file_identifier,
        },
    };
    Ok((canonical_directory_path, canonical_path_text, filesystem_identity))
}

pub(super) fn filesystem_identity_columns(
    filesystem_identity: &ProjectFilesystemIdentity,
) -> (&'static str, Vec<u8>, Vec<u8>) {
    match filesystem_identity {
        ProjectFilesystemIdentity::Unix {
            device_identifier,
            inode_identifier,
        } => (
            "unix",
            device_identifier.to_le_bytes().to_vec(),
            inode_identifier.to_le_bytes().to_vec(),
        ),
        ProjectFilesystemIdentity::Windows {
            volume_identifier,
            file_identifier,
        } => ("windows", volume_identifier.clone(), file_identifier.to_vec()),
    }
}

pub(super) fn invalid_project_path(path: &Path, reason: impl std::fmt::Display) -> InfrastructureError {
    InfrastructureError::new(
        InfrastructureErrorKind::InvalidProjectPath,
        format!("invalid project directory '{}': {reason}", path.display()),
    )
}
