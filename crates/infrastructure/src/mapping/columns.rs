use crate::database::database_operation_error;
use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult, LlmMessageState};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

pub(super) fn parse_llm_message_state(stored_state: &str) -> InfrastructureResult<LlmMessageState> {
    match stored_state {
        "in_progress" => Ok(LlmMessageState::InProgress),
        "completed" => Ok(LlmMessageState::Completed),
        "failed" => Ok(LlmMessageState::Failed),
        "cancelled" => Ok(LlmMessageState::Cancelled),
        unexpected_state => Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored LLM message state '{unexpected_state}' is invalid"),
        )),
    }
}

pub(super) fn read_column<'database_row, ColumnValue>(
    database_row: &'database_row SqliteRow,
    column_name: &str,
) -> InfrastructureResult<ColumnValue>
where
    ColumnValue: sqlx::Decode<'database_row, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    database_row
        .try_get(column_name)
        .map_err(|source| database_operation_error("failed to decode database row", source))
}

pub(super) fn read_required_optional_column<'database_row, ColumnValue>(
    database_row: &'database_row SqliteRow,
    column_name: &str,
) -> InfrastructureResult<ColumnValue>
where
    ColumnValue: sqlx::Decode<'database_row, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    read_column::<Option<ColumnValue>>(database_row, column_name)?.ok_or_else(|| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("required role-specific column '{column_name}' is null"),
        )
    })
}

pub(super) fn read_integer_boolean(
    database_row: &SqliteRow,
    column_name: &str,
) -> InfrastructureResult<bool> {
    match read_column::<i64>(database_row, column_name)? {
        0 => Ok(false),
        1 => Ok(true),
        stored_value => Err(corrupt_database_value(
            column_name,
            format!("expected 0 or 1, found {stored_value}"),
        )),
    }
}

pub(super) fn read_u8(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<u8> {
    let stored_value: i64 = read_column(database_row, column_name)?;
    let value = u8::try_from(stored_value)
        .map_err(|source| corrupt_database_value(column_name, source.to_string()))?;

    if value > 100 {
        return Err(corrupt_database_value(
            column_name,
            format!("expected a percentage through 100, found {value}"),
        ));
    }

    Ok(value)
}

pub(super) fn corrupt_database_value(
    column_name: &str,
    detail: impl std::fmt::Display,
) -> InfrastructureError {
    InfrastructureError::new(
        InfrastructureErrorKind::CorruptDatabaseValue,
        format!("stored column '{column_name}' is corrupt: {detail}"),
    )
}

pub(super) fn read_u32(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<u32> {
    let stored_value: i64 = read_column(database_row, column_name)?;

    u32::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::CorruptDatabaseValue,
            format!("stored column '{column_name}' is outside the u32 range: {source}"),
        )
    })
}

pub(super) fn read_required_optional_u32(
    database_row: &SqliteRow,
    column_name: &str,
) -> InfrastructureResult<u32> {
    let stored_value: i64 = read_required_optional_column(database_row, column_name)?;

    u32::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::CorruptDatabaseValue,
            format!("stored column '{column_name}' is outside the u32 range: {source}"),
        )
    })
}

pub(super) fn read_nonnegative_u64(
    database_row: &SqliteRow,
    column_name: &str,
) -> InfrastructureResult<u64> {
    let stored_value: i64 = read_column(database_row, column_name)?;

    u64::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored column '{column_name}' is negative: {source}"),
        )
    })
}
