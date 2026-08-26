use super::*;

pub(crate) struct SqlCondition {
    pub(crate) connector: Option<&'static str>,
    pub(crate) statement: String,
    pub(crate) values: Vec<SqlValue>,
}

pub(crate) struct SqlJoin {
    pub(crate) join_type: &'static str,
    pub(crate) table: String,
    pub(crate) alias: Option<String>,
    pub(crate) condition: Option<String>,
    pub(crate) values: Vec<SqlValue>,
}

pub struct SelectSqlBuilder<'target> {
    pub(crate) execution_target: SqlExecutionTarget<'target>,
    pub(crate) selections: Vec<String>,
    pub(crate) source: Option<String>,
    pub(crate) joins: Vec<SqlJoin>,
    pub(crate) filters: Vec<SqlCondition>,
    pub(crate) group_columns: Vec<String>,
    pub(crate) having_conditions: Vec<SqlCondition>,
    pub(crate) order_columns: Vec<(String, SqlSortDirection)>,
    pub(crate) distinct: bool,
    pub(crate) limit: Option<u64>,
    pub(crate) offset: Option<u64>,
    pub(crate) validation_error: Option<InfrastructureError>,
}

impl<'target> SelectSqlBuilder<'target> {
    pub(crate) fn new(execution_target: SqlExecutionTarget<'target>) -> Self {
        Self {
            execution_target,
            selections: Vec::new(),
            source: None,
            joins: Vec::new(),
            filters: Vec::new(),
            group_columns: Vec::new(),
            having_conditions: Vec::new(),
            order_columns: Vec::new(),
            distinct: false,
            limit: None,
            offset: None,
            validation_error: None,
        }
    }

    pub fn select<Columns, ColumnName>(mut self, column_names: Columns) -> Self
    where
        Columns: IntoIterator<Item = ColumnName>,
        ColumnName: AsRef<str>,
    {
        for column_name in column_names {
            self.push_selection_identifier(column_name.as_ref());
        }
        self
    }

    pub fn select_as(mut self, column_name: impl AsRef<str>, result_name: impl AsRef<str>) -> Self {
        match (
            quote_qualified_identifier(column_name.as_ref(), true),
            quote_identifier_segment(result_name.as_ref()),
        ) {
            (Ok(column_name), Ok(result_name)) => self
                .selections
                .push(format!("{column_name} AS {result_name}")),
            (Err(error), _) | (_, Err(error)) => self.store_validation_error(error),
        }
        self
    }

    pub fn select_all(mut self, table_alias: impl AsRef<str>) -> Self {
        self.push_selection_identifier(&format!("{}.*", table_alias.as_ref()));
        self
    }

    pub fn trusted_raw_expression(mut self, expression: impl Into<String>) -> Self {
        match validate_single_statement_fragment(expression.into()) {
            Ok(expression) => self.selections.push(expression),
            Err(error) => self.store_validation_error(error),
        }
        self
    }

    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    pub fn from(mut self, table_name: impl AsRef<str>) -> Self {
        match quote_qualified_identifier(table_name.as_ref(), false) {
            Ok(table_name) => self.source = Some(table_name),
            Err(error) => self.store_validation_error(error),
        }
        self
    }

    pub fn from_as(mut self, table_name: impl AsRef<str>, table_alias: impl AsRef<str>) -> Self {
        match (
            quote_qualified_identifier(table_name.as_ref(), false),
            quote_identifier_segment(table_alias.as_ref()),
        ) {
            (Ok(table_name), Ok(table_alias)) => {
                self.source = Some(format!("{table_name} AS {table_alias}"))
            }
            (Err(error), _) | (_, Err(error)) => self.store_validation_error(error),
        }
        self
    }

    pub fn inner_join(self, table_name: impl AsRef<str>) -> Self {
        self.add_join("INNER JOIN", table_name.as_ref(), None)
    }

    pub fn inner_join_as(self, table_name: impl AsRef<str>, table_alias: impl AsRef<str>) -> Self {
        self.add_join(
            "INNER JOIN",
            table_name.as_ref(),
            Some(table_alias.as_ref()),
        )
    }

    pub fn left_join(self, table_name: impl AsRef<str>) -> Self {
        self.add_join("LEFT JOIN", table_name.as_ref(), None)
    }

    pub fn left_join_as(self, table_name: impl AsRef<str>, table_alias: impl AsRef<str>) -> Self {
        self.add_join("LEFT JOIN", table_name.as_ref(), Some(table_alias.as_ref()))
    }

    pub fn on(
        mut self,
        left_column_name: impl AsRef<str>,
        right_column_name: impl AsRef<str>,
    ) -> Self {
        let condition = quote_qualified_identifier(left_column_name.as_ref(), false).and_then(
            |left_column_name| {
                quote_qualified_identifier(right_column_name.as_ref(), false)
                    .map(|right_column_name| format!("{left_column_name} = {right_column_name}"))
            },
        );

        match condition {
            Ok(condition) => match self.joins.last_mut() {
                Some(join) => join.condition = Some(condition),
                None => self.store_validation_error(sql_builder_error(
                    "join condition requires a preceding join",
                )),
            },
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub fn on_condition<Values>(
        mut self,
        condition_template: impl AsRef<str>,
        values: Values,
    ) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        match compile_bound_condition_template(condition_template.as_ref(), values) {
            Ok((condition, values)) => match self.joins.last_mut() {
                Some(join) => {
                    join.condition = Some(condition);
                    join.values = values;
                }
                None => self.store_validation_error(sql_builder_error(
                    "join condition requires a preceding join",
                )),
            },
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub fn filter<Values>(self, condition_template: impl AsRef<str>, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        self.add_filter(None, condition_template.as_ref(), values)
    }

    pub fn and_filter<Values>(self, condition_template: impl AsRef<str>, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        self.add_filter(Some("AND"), condition_template.as_ref(), values)
    }

    pub fn or_filter<Values>(self, condition_template: impl AsRef<str>, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        self.add_filter(Some("OR"), condition_template.as_ref(), values)
    }

    pub fn group_by(mut self, column_name: impl AsRef<str>) -> Self {
        match quote_qualified_identifier(column_name.as_ref(), false) {
            Ok(column_name) => self.group_columns.push(column_name),
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub fn having<Values>(mut self, condition_template: impl AsRef<str>, values: Values) -> Self
    where
        Values: IntoIterator<Item = SqlValue>,
    {
        match compile_bound_condition_template(condition_template.as_ref(), values) {
            Ok((statement, values)) => self.having_conditions.push(SqlCondition {
                connector: None,
                statement,
                values,
            }),
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub fn order_by(mut self, column_name: impl AsRef<str>, direction: SqlSortDirection) -> Self {
        match quote_qualified_identifier(column_name.as_ref(), false) {
            Ok(column_name) => self.order_columns.push((column_name, direction)),
            Err(error) => self.store_validation_error(error),
        }

        self
    }

    pub fn limit(mut self, row_limit: u64) -> Self {
        self.limit = Some(row_limit);
        self
    }

    pub fn offset(mut self, row_offset: u64) -> Self {
        self.offset = Some(row_offset);
        self
    }
}
