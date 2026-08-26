use super::*;

impl<'target> SelectSqlBuilder<'target> {
    pub async fn commit(self) -> InfrastructureResult<Vec<SqlRow>> {
        let (mut execution_target, statement, values) = self.compile()?;
        execute_select_with_automatic_commit(&mut execution_target, statement, values).await
    }

    pub async fn fetch_all(self) -> InfrastructureResult<Vec<SqlRow>> {
        let (mut execution_target, statement, values) = self.compile()?;
        execute_select_in_existing_transaction(&mut execution_target, statement, values).await
    }

    pub async fn fetch_optional(mut self) -> InfrastructureResult<Option<SqlRow>> {
        self.limit = Some(2);
        let (mut execution_target, statement, values) = self.compile()?;
        let mut rows =
            execute_select_in_existing_transaction(&mut execution_target, statement, values)
                .await?;

        if rows.len() > 1 {
            execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error(
                "fetch_optional returned more than one SQL row",
            ));
        }

        Ok(rows.pop())
    }

    pub async fn fetch_one(mut self) -> InfrastructureResult<SqlRow> {
        self.limit = Some(2);
        let (mut execution_target, statement, values) = self.compile()?;
        let mut rows =
            execute_select_in_existing_transaction(&mut execution_target, statement, values)
                .await?;

        if rows.len() > 1 {
            execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error(
                "fetch_one returned more than one SQL row",
            ));
        }

        rows.pop().ok_or_else(|| {
            execution_target.mark_transaction_as_failed();
            sql_builder_error("fetch_one did not return a SQL row")
        })
    }

    pub(crate) fn push_selection_identifier(&mut self, column_name: &str) {
        match quote_qualified_identifier(column_name, true) {
            Ok(column_name) => self.selections.push(column_name),
            Err(error) => self.store_validation_error(error),
        }
    }

    pub(crate) fn add_join(
        mut self,
        join_type: &'static str,
        table_name: &str,
        table_alias: Option<&str>,
    ) -> Self {
        let quoted_table_name = quote_qualified_identifier(table_name, false);
        let quoted_table_alias = table_alias.map(quote_identifier_segment).transpose();

        match (quoted_table_name, quoted_table_alias) {
            (Ok(table), Ok(alias)) => self.joins.push(SqlJoin {
                join_type,
                table,
                alias,
                condition: None,
                values: Vec::new(),
            }),
            (Err(error), _) | (_, Err(error)) => self.store_validation_error(error),
        }

        self
    }

    pub(crate) fn add_filter<Values>(
        mut self,
        connector: Option<&'static str>,
        condition_template: &str,
        values: Values,
    ) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        match compile_bound_condition_template(condition_template, values) {
            Ok((statement, values)) => self.filters.push(SqlCondition {
                connector,
                statement,
                values,
            }),
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub(crate) fn store_validation_error(&mut self, error: InfrastructureError) {
        if self.validation_error.is_none() {
            self.validation_error = Some(error);
        }
    }

    fn compile(
        mut self,
    ) -> InfrastructureResult<(SqlExecutionTarget<'target>, String, Vec<SqlValue>)> {
        if let Some(error) = self.validation_error.take() {
            self.execution_target.mark_transaction_as_failed();
            return Err(error);
        }

        if self.selections.is_empty() {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error("SELECT requires at least one column"));
        }

        let Some(source) = self.source.take() else {
            self.execution_target.mark_transaction_as_failed();
            return Err(sql_builder_error("SELECT requires a FROM table"));
        };
        let mut statement = String::from("SELECT ");

        if self.distinct {
            statement.push_str("DISTINCT ");
        }

        statement.push_str(&self.selections.join(", "));
        statement.push_str(" FROM ");
        statement.push_str(&source);
        let mut values = Vec::new();

        for join in self.joins {
            statement.push(' ');
            statement.push_str(join.join_type);
            statement.push(' ');
            statement.push_str(&join.table);
            if let Some(alias) = join.alias {
                statement.push_str(" AS ");
                statement.push_str(&alias);
            }
            let Some(condition) = join.condition else {
                self.execution_target.mark_transaction_as_failed();
                return Err(sql_builder_error("every join requires an ON condition"));
            };
            statement.push_str(" ON ");
            statement.push_str(&condition);
            values.extend(join.values);
        }

        append_conditions(&mut statement, " WHERE ", self.filters, &mut values);

        if !self.group_columns.is_empty() {
            statement.push_str(" GROUP BY ");
            statement.push_str(&self.group_columns.join(", "));
        }

        append_conditions(
            &mut statement,
            " HAVING ",
            self.having_conditions,
            &mut values,
        );

        if !self.order_columns.is_empty() {
            statement.push_str(" ORDER BY ");
            statement.push_str(
                &self
                    .order_columns
                    .into_iter()
                    .map(|(column_name, direction)| {
                        let direction_text = match direction {
                            SqlSortDirection::Ascending => "ASC",
                            SqlSortDirection::Descending => "DESC",
                        };
                        format!("{column_name} {direction_text}")
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        if let Some(row_limit) = self.limit {
            statement.push_str(&format!(" LIMIT {row_limit}"));
        }

        if let Some(row_offset) = self.offset {
            if self.limit.is_none() {
                statement.push_str(" LIMIT -1");
            }
            statement.push_str(&format!(" OFFSET {row_offset}"));
        }

        Ok((self.execution_target, statement, values))
    }
}
