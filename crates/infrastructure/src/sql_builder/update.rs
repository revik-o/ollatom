use super::*;

pub struct UpdateSqlBuilder<'target> {
    execution_target: SqlExecutionTarget<'target>,
    table_name: Result<String, InfrastructureError>,
    assignments: Vec<(String, SqlValue)>,
    filters: Vec<SqlCondition>,
    returning_columns: Vec<String>,
    allow_all_rows: bool,
    validation_error: Option<InfrastructureError>,
}

impl<'target> UpdateSqlBuilder<'target> {
    pub(crate) fn new(execution_target: SqlExecutionTarget<'target>, table_name: &str) -> Self {
        Self {
            execution_target,
            table_name: quote_qualified_identifier(table_name, false),
            assignments: Vec::new(),
            filters: Vec::new(),
            returning_columns: Vec::new(),
            allow_all_rows: false,
            validation_error: None,
        }
    }

    pub fn set(mut self, column_name: impl AsRef<str>, value: impl Into<SqlValue>) -> Self {
        match quote_identifier_segment(column_name.as_ref()) {
            Ok(column_name) => self.assignments.push((column_name, value.into())),
            Err(error) => self.store_validation_error(error),
        }
        self
    }

    pub fn filter<Values>(mut self, condition_template: impl AsRef<str>, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        match compile_bound_condition_template(condition_template.as_ref(), values) {
            Ok((statement, values)) => self.filters.push(SqlCondition {
                connector: None,
                statement,
                values,
            }),
            Err(error) => self.store_validation_error(error),
        }

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

    pub fn allow_all_rows(mut self) -> Self {
        self.allow_all_rows = true;
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
            Err(sql_builder_error("UPDATE builder has already compiled")),
        ) {
            Ok(table_name) => table_name,
            Err(error) => {
                self.execution_target.mark_transaction_as_failed();
                return Err(error);
            }
        };

        if self.assignments.is_empty() {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error("UPDATE requires at least one assignment"));
        }

        if self.filters.is_empty() && !self.allow_all_rows {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error(
                "UPDATE without filters requires allow_all_rows",
            ));
        }

        let mut statement = format!(
            "UPDATE {table_name} SET {}",
            self.assignments
                .iter()
                .map(|(column_name, _)| format!("{column_name} = ?"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut values = self
            .assignments
            .into_iter()
            .map(|(_, value)| value)
            .collect();
        append_conditions(&mut statement, " WHERE ", self.filters, &mut values);
        let returns_rows = !self.returning_columns.is_empty();
        append_returning_clause(&mut statement, self.returning_columns);

        Ok((self.execution_target, statement, values, returns_rows))
    }
}
