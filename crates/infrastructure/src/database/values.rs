use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use sqlx::error::ErrorKind;
use std::path::{Path, PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub(crate) fn database_operation_error(operation: &str, source: sqlx::Error) -> InfrastructureError {
    let error_kind = match &source {
        sqlx::Error::Database(database_error)
            if matches!(
                database_error.kind(),
                ErrorKind::UniqueViolation
                    | ErrorKind::ForeignKeyViolation
                    | ErrorKind::NotNullViolation
                    | ErrorKind::CheckViolation
            ) =>
        {
            InfrastructureErrorKind::DatabaseConstraintViolation
        }
        _ => InfrastructureErrorKind::DatabaseOperationFailed,
    };

    InfrastructureError::from_database_error(error_kind, operation, source)
}

pub(crate) fn format_timestamp(timestamp: OffsetDateTime) -> InfrastructureResult<String> {
    timestamp.format(&Rfc3339).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to format UTC timestamp: {source}"),
        )
    })
}

pub(crate) fn parse_timestamp(timestamp: &str) -> InfrastructureResult<OffsetDateTime> {
    OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to parse stored UTC timestamp '{timestamp}': {source}"),
        )
    })
}

pub(crate) fn parse_optional_timestamp(timestamp: Option<String>) -> InfrastructureResult<Option<OffsetDateTime>> {
    timestamp.as_deref().map(parse_timestamp).transpose()
}

pub(crate) fn parse_uuid(identifier_bytes: Vec<u8>) -> InfrastructureResult<Uuid> {
    Uuid::from_slice(&identifier_bytes).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to decode stored UUID: {source}"),
        )
    })
}

pub(crate) fn validate_nonblank_value(field_name: &str, field_value: &str) -> InfrastructureResult<()> {
    if field_value.trim().is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            format!("{field_name} must not be blank"),
        ));
    }

    Ok(())
}

pub(crate) fn validate_usage_percentage(field_name: &str, usage_percentage: u8) -> InfrastructureResult<()> {
    if usage_percentage > 100 {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            format!("{field_name} must be between 0 and 100"),
        ));
    }

    Ok(())
}

pub(crate) fn clone_database_file_path(database_file_path: &Path) -> PathBuf {
    database_file_path.to_owned()
}
