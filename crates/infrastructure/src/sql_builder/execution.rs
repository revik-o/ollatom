use super::*;

pub(crate) async fn execute_select_with_automatic_commit(
    execution_target: &mut SqlExecutionTarget<'_>,
    statement: String,
    values: Vec<SqlValue>,
) -> InfrastructureResult<Vec<SqlRow>> {
    let SqlExecutionTarget::Infrastructure(infrastructure) = execution_target else {
        execution_target.mark_transaction_as_failed();
        return Err(sql_builder_error(
            "commit is only available on Infrastructure::sql_builder",
        ));
    };
    let mut database_transaction = infrastructure
        .connection_pool
        .begin()
        .await
        .map_err(|source| database_operation_error("failed to begin read transaction", source))?;
    let operation_result = fetch_sql_rows(&mut database_transaction, statement, values).await;

    match operation_result {
        Ok(rows) => {
            database_transaction.commit().await.map_err(|source| {
                database_operation_error("failed to commit read transaction", source)
            })?;
            Ok(rows)
        }
        Err(error) => match database_transaction.rollback().await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(InfrastructureError::new(
                InfrastructureErrorKind::DatabaseActionAndRollbackFailed,
                format!("SQL builder action failed: {error}; rollback failed: {rollback_error}"),
            )),
        },
    }
}

pub(crate) async fn execute_select_in_existing_transaction(
    execution_target: &mut SqlExecutionTarget<'_>,
    statement: String,
    values: Vec<SqlValue>,
) -> InfrastructureResult<Vec<SqlRow>> {
    let SqlExecutionTarget::Transaction(transaction) = execution_target else {
        return Err(sql_builder_error(
            "fetch operations are only available on InfrastructureTransaction::sql_builder",
        ));
    };
    let operation_result =
        fetch_sql_rows(transaction.database_transaction_mut()?, statement, values).await;

    if operation_result.is_err() {
        transaction.mark_operation_as_failed();
    }

    operation_result
}

pub(crate) async fn execute_mutation_with_automatic_commit(
    mut execution_target: SqlExecutionTarget<'_>,
    statement: String,
    values: Vec<SqlValue>,
    returns_rows: bool,
) -> InfrastructureResult<SqlMutationResult> {
    let SqlExecutionTarget::Infrastructure(infrastructure) = &mut execution_target else {
        execution_target.mark_transaction_as_failed();
        return Err(sql_builder_error(
            "commit is only available on Infrastructure::sql_builder",
        ));
    };
    let mut transaction = infrastructure.make_transaction().await?;
    let operation_result = execute_sql_mutation(
        transaction.database_transaction_mut()?,
        statement,
        values,
        returns_rows,
    )
    .await;

    match operation_result {
        Ok(mutation_result) => {
            transaction.commit().await?;
            Ok(mutation_result)
        }
        Err(error) => {
            transaction.mark_operation_as_failed();
            match transaction.rollback().await {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(InfrastructureError::new(
                    InfrastructureErrorKind::DatabaseActionAndRollbackFailed,
                    format!(
                        "SQL builder action failed: {error}; rollback failed: {rollback_error}"
                    ),
                )),
            }
        }
    }
}

pub(crate) async fn execute_mutation_in_existing_transaction(
    mut execution_target: SqlExecutionTarget<'_>,
    statement: String,
    values: Vec<SqlValue>,
    returns_rows: bool,
) -> InfrastructureResult<SqlMutationResult> {
    let SqlExecutionTarget::Transaction(transaction) = &mut execution_target else {
        return Err(sql_builder_error(
            "execute is only available on InfrastructureTransaction::sql_builder",
        ));
    };
    let operation_result = execute_sql_mutation(
        transaction.database_transaction_mut()?,
        statement,
        values,
        returns_rows,
    )
    .await;

    if operation_result.is_err() {
        transaction.mark_operation_as_failed();
    }
    operation_result
}

pub(crate) async fn fetch_sql_rows(
    database_transaction: &mut sqlx::Transaction<'static, Sqlite>,
    statement: String,
    values: Vec<SqlValue>,
) -> InfrastructureResult<Vec<SqlRow>> {
    let query = bind_sql_values(sqlx::query(AssertSqlSafe(statement)), values);
    let database_rows = query
        .fetch_all(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("SQL builder query failed", source))?;
    database_rows
        .iter()
        .map(convert_sqlite_row_to_sql_row)
        .collect()
}

pub(crate) async fn execute_sql_mutation(
    database_transaction: &mut sqlx::Transaction<'static, Sqlite>,
    statement: String,
    values: Vec<SqlValue>,
    returns_rows: bool,
) -> InfrastructureResult<SqlMutationResult> {
    if returns_rows {
        let returned_rows = fetch_sql_rows(database_transaction, statement, values).await?;
        return Ok(SqlMutationResult {
            rows_affected: returned_rows.len() as u64,
            returned_rows,
        });
    }

    let query_result = bind_sql_values(sqlx::query(AssertSqlSafe(statement)), values)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("SQL builder mutation failed", source))?;

    Ok(SqlMutationResult {
        rows_affected: query_result.rows_affected(),
        returned_rows: Vec::new(),
    })
}
