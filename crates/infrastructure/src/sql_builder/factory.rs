use super::*;

pub(crate) enum SqlExecutionTarget<'target> {
    Infrastructure(&'target Infrastructure),
    Transaction(&'target mut InfrastructureTransaction),
}

impl SqlExecutionTarget<'_> {
    pub(crate) fn mark_transaction_as_failed(&mut self) {
        if let Self::Transaction(transaction) = self {
            transaction.mark_operation_as_failed();
        }
    }
}

pub struct SqlBuilderFactory<'target> {
    execution_target: SqlExecutionTarget<'target>,
}

impl<'target> SqlBuilderFactory<'target> {
    pub(crate) fn for_infrastructure(infrastructure: &'target Infrastructure) -> Self {
        Self {
            execution_target: SqlExecutionTarget::Infrastructure(infrastructure),
        }
    }

    pub(crate) fn for_transaction(transaction: &'target mut InfrastructureTransaction) -> Self {
        Self {
            execution_target: SqlExecutionTarget::Transaction(transaction),
        }
    }

    pub fn select<Columns, ColumnName>(self, column_names: Columns) -> SelectSqlBuilder<'target>
    where
        Columns: IntoIterator<Item = ColumnName>,
        ColumnName: AsRef<str>,
    {
        SelectSqlBuilder::new(self.execution_target).select(column_names)
    }

    pub fn select_as(
        self,
        column_name: impl AsRef<str>,
        result_name: impl AsRef<str>,
    ) -> SelectSqlBuilder<'target> {
        SelectSqlBuilder::new(self.execution_target).select_as(column_name, result_name)
    }

    pub fn select_all(self, table_alias: impl AsRef<str>) -> SelectSqlBuilder<'target> {
        SelectSqlBuilder::new(self.execution_target).select_all(table_alias)
    }

    pub fn trusted_raw_expression(
        self,
        expression: impl Into<String>,
    ) -> SelectSqlBuilder<'target> {
        SelectSqlBuilder::new(self.execution_target).trusted_raw_expression(expression)
    }

    pub fn insert_into(self, table_name: impl AsRef<str>) -> InsertSqlBuilder<'target> {
        InsertSqlBuilder::new(self.execution_target, table_name.as_ref())
    }

    pub fn update(self, table_name: impl AsRef<str>) -> UpdateSqlBuilder<'target> {
        UpdateSqlBuilder::new(self.execution_target, table_name.as_ref())
    }

    pub fn delete_from(self, table_name: impl AsRef<str>) -> DeleteSqlBuilder<'target> {
        DeleteSqlBuilder::new(self.execution_target, table_name.as_ref())
    }
}
