use super::*;

pub struct InsertSqlBuilder<'target> {
    execution_target: SqlExecutionTarget<'target>,
    table_name: Result<String, InfrastructureError>,
    column_names: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
    returning_columns: Vec<String>,
    validation_error: Option<InfrastructureError>,
}

impl<'target> InsertSqlBuilder<'target> {
    pub(crate) fn new(execution_target: SqlExecutionTarget<'target>, table_name: &str) -> Self {
        Self {
            execution_target,
            table_name: quote_qualified_identifier(table_name, false),
            column_names: Vec::new(),
            rows: Vec::new(),
            returning_columns: Vec::new(),
            validation_error: None,
        }
    }

    pub fn columns<Columns, ColumnName>(mut self, column_names: Columns) -> Self
    where
        Columns: IntoIterator<Item = ColumnName>,
        ColumnName: AsRef<str>,
    {
        for column_name in column_names {
            match quote_identifier_segment(column_name.as_ref()) {
                Ok(column_name) => self.column_names.push(column_name),
                Err(error) => self.store_validation_error(error),
            }
        }

        self
    }

    pub fn values<Values>(mut self, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        self.rows.push(values.into_iter().collect());
        self
    }

    pub fn returning<Columns, ColumnName>(mut self, column_names: Columns) -> Self
    where
        Columns: IntoIterator<Item = ColumnName>,
        ColumnName: AsRef<str>,
    {
        add_returning_columns(
            &mut self.returning_columns,
            &mut self.validation_error,
            column_names,
        );
        self
    }

    pub async fn commit(self) -> InfrastructureResult<SqlMutationResult> {
        let (execution_target, statement, values, returns_rows) = self.compile()?;
        execute_mutation_with_automatic_commit(execution_target, statement, values, returns_rows)
            .await
    }

    pub async fn execute(self) -> InfrastructureResult<SqlMutationResult> {
        let (execution_target, statement, values, returns_rows) = self.compile()?;
        execute_mutation_in_existing_transaction(execution_target, statement, values, returns_rows)
            .await
    }

    fn store_validation_error(&mut self, error: InfrastructureError) {
        if self.validation_error.is_none() {
            self.validation_error = Some(error);
        }
    }

    fn compile(
        mut self,
    ) -> InfrastructureResult<(SqlExecutionTarget<'target>, String, Vec<SqlValue>, bool)> {
        if let Some(error) = self.validation_error.take() {
            self.execution_target.mark_transaction_as_failed();
            return Err(error);
        }

        let table_name = match std::mem::replace(
            &mut self.table_name,
            Err(sql_builder_error("INSERT builder has already compiled")),
        ) {
            Ok(table_name) => table_name,
            Err(error) => {
                self.execution_target.mark_transaction_as_failed();
                return Err(error);
            }
        };

        if self.column_names.is_empty() || self.rows.is_empty() {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error(
                "INSERT requires columns and at least one values row",
            ));
        }

        if self
            .rows
            .iter()
            .any(|row| row.len() != self.column_names.len())
        {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error(
                "every INSERT values row must match the column count",
            ));
        }

        let row_placeholders = format!(
            "({})",
            std::iter::repeat_n("?", self.column_names.len())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut statement = format!(
            "INSERT INTO {table_name} ({}) VALUES {}",
            self.column_names.join(", "),
            std::iter::repeat_n(row_placeholders, self.rows.len())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let values = self.rows.into_iter().flatten().collect();
        let returns_rows = !self.returning_columns.is_empty();
        append_returning_clause(&mut statement, self.returning_columns);
        Ok((self.execution_target, statement, values, returns_rows))
    }
}
