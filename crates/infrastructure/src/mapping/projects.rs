use super::columns::{corrupt_database_value, read_column};
use super::settings::map_execution_settings;
use crate::database::{parse_timestamp, parse_uuid};
use crate::{InfrastructureResult, Project, ProjectFilesystemIdentity, ProjectId};
use sqlx::sqlite::SqliteRow;

pub(crate) fn map_project_row(database_row: &SqliteRow) -> InfrastructureResult<Project> {
    let filesystem_identity_kind: String = read_column(database_row, "filesystem_identity_kind")?;
    let filesystem_volume_identifier: Vec<u8> = read_column(database_row, "filesystem_volume_identifier")?;
    let filesystem_object_identifier: Vec<u8> = read_column(database_row, "filesystem_object_identifier")?;

    let filesystem_identity = match filesystem_identity_kind.as_str() {
        "unix" if filesystem_volume_identifier.len() == 8 && filesystem_object_identifier.len() == 8 => {
            ProjectFilesystemIdentity::Unix {
                device_identifier: u64::from_le_bytes(filesystem_volume_identifier.as_slice().try_into().map_err(
                    |_| {
                        corrupt_database_value(
                            "filesystem_volume_identifier",
                            "expected 8 bytes for Unix device identity",
                        )
                    },
                )?),
                inode_identifier: u64::from_le_bytes(filesystem_object_identifier.as_slice().try_into().map_err(
                    |_| {
                        corrupt_database_value(
                            "filesystem_object_identifier",
                            "expected 8 bytes for Unix inode identity",
                        )
                    },
                )?),
            }
        }
        "windows" if filesystem_volume_identifier.len() == 8 && filesystem_object_identifier.len() == 16 => {
            ProjectFilesystemIdentity::Windows {
                volume_identifier: filesystem_volume_identifier,
                file_identifier: filesystem_object_identifier.try_into().map_err(|_| {
                    corrupt_database_value(
                        "filesystem_object_identifier",
                        "expected 16 bytes for Windows file identity",
                    )
                })?,
            }
        }
        unexpected_kind => {
            return Err(corrupt_database_value(
                "filesystem_identity_kind",
                format!("unsupported identity kind '{unexpected_kind}' or invalid identifier lengths"),
            ));
        }
    };

    Ok(Project {
        id: ProjectId::from(parse_uuid(read_column(database_row, "id")?)?),
        name: read_column(database_row, "name")?,
        canonical_directory_path: std::path::PathBuf::from(read_column::<String>(
            database_row,
            "canonical_directory_path",
        )?),
        filesystem_identity,
        execution_settings: map_execution_settings(database_row)?,
        created_at: parse_timestamp(read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_timestamp(read_column::<&str>(database_row, "updated_at")?)?,
    })
}
