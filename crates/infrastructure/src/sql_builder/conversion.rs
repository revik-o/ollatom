use super::*;

pub(crate) fn bind_sql_values(
    mut query: sqlx::query::Query<'static, Sqlite, SqliteArguments>,
    values: Vec<SqlValue>,
) -> sqlx::query::Query<'static, Sqlite, SqliteArguments> {
    for value in values {
        query = match value {
            SqlValue::Null => query.bind(Option::<String>::None),
            SqlValue::Integer(value) => query.bind(value),
            SqlValue::Real(value) => query.bind(value),
            SqlValue::Text(value) => query.bind(value),
            SqlValue::Blob(value) => query.bind(value),
        };
    }
    query
}

pub(crate) fn convert_sqlite_row_to_sql_row(
    database_row: &SqliteRow,
) -> InfrastructureResult<SqlRow> {
    let mut columns = Vec::with_capacity(database_row.len());
    for (column_index, column) in database_row.columns().iter().enumerate() {
        let raw_value = database_row
            .try_get_raw(column_index)
            .map_err(|source| database_operation_error("failed to read SQL row value", source))?;
        let value = if raw_value.is_null() {
            SqlValue::Null
        } else {
            match raw_value.type_info().name() {
                "INTEGER" | "INT" => {
                    SqlValue::Integer(database_row.try_get(column_index).map_err(|source| {
                        database_operation_error("failed to decode integer SQL value", source)
                    })?)
                }
                "REAL" | "FLOAT" | "DOUBLE" => {
                    SqlValue::Real(database_row.try_get(column_index).map_err(|source| {
                        database_operation_error("failed to decode real SQL value", source)
                    })?)
                }
                "BLOB" => SqlValue::Blob(database_row.try_get(column_index).map_err(|source| {
                    database_operation_error("failed to decode blob SQL value", source)
                })?),
                _ => SqlValue::Text(database_row.try_get(column_index).map_err(|source| {
                    database_operation_error("failed to decode text SQL value", source)
                })?),
            }
        };
        columns.push(SqlColumn {
            name: column.name().to_owned(),
            value,
        });
    }
    Ok(SqlRow { columns })
}

pub(crate) fn sql_value_to_json(value: SqlValue) -> InfrastructureResult<Value> {
    match value {
        SqlValue::Null => Ok(Value::Null),
        SqlValue::Integer(value) => Ok(Value::Number(Number::from(value))),
        SqlValue::Real(value) => Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| sql_builder_error("real SQL value cannot be represented as JSON")),
        SqlValue::Text(value) => Ok(Value::String(value)),
        SqlValue::Blob(value) => Ok(Value::Array(
            value
                .into_iter()
                .map(|byte| Value::Number(Number::from(byte)))
                .collect(),
        )),
    }
}

pub(crate) fn unexpected_sql_value(
    expected_type: &str,
    actual_value: &SqlValue,
) -> InfrastructureError {
    sql_builder_error(format!(
        "expected {expected_type} SQL value but received {actual_value:?}"
    ))
}

pub(crate) fn sql_builder_error(message: impl Into<String>) -> InfrastructureError {
    InfrastructureError::new(InfrastructureErrorKind::InvalidSqlBuilderOperation, message)
}
