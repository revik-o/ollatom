use crate::InfrastructureResult;
use crate::database::{database_operation_error, format_timestamp, validate_usage_percentage};
use sqlx::{AssertSqlSafe, Sqlite, Transaction};
use time::OffsetDateTime;

pub(crate) async fn set_boolean_entity_value(
    database_transaction: &mut Transaction<'static, Sqlite>,
    table_name: &str,
    column_name: &str,
    column_value: bool,
    entity_id: Vec<u8>,
) -> InfrastructureResult<bool> {
    let updated_at = format_timestamp(OffsetDateTime::now_utc())?;
    let query_text =
        format!("UPDATE {table_name} SET {column_name} = ?, updated_at = ? WHERE id = ?");
    let query_result = sqlx::query(AssertSqlSafe(query_text))
        .bind(column_value)
        .bind(updated_at)
        .bind(entity_id)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to update boolean setting", source))?;

    Ok(query_result.rows_affected() > 0)
}

pub(crate) async fn set_usage_entity_value(
    database_transaction: &mut Transaction<'static, Sqlite>,
    table_name: &str,
    column_name: &str,
    column_value: u8,
    entity_id: Vec<u8>,
) -> InfrastructureResult<bool> {
    validate_usage_percentage(column_name, column_value)?;
    let updated_at = format_timestamp(OffsetDateTime::now_utc())?;
    let query_text =
        format!("UPDATE {table_name} SET {column_name} = ?, updated_at = ? WHERE id = ?");
    let query_result = sqlx::query(AssertSqlSafe(query_text))
        .bind(column_value as i64)
        .bind(updated_at)
        .bind(entity_id)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to update usage setting", source))?;

    Ok(query_result.rows_affected() > 0)
}
