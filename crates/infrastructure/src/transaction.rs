mod chat_api;
mod message_api;
mod project_api;
mod store;

use crate::database::database_operation_error;
use crate::{
    ChatId, InfrastructureError, InfrastructureErrorKind, InfrastructureResult, ProjectId,
    SqlBuilderFactory,
};
use sqlx::{Sqlite, SqlitePool, Transaction};
use store::{set_boolean_entity_value, set_usage_entity_value};

pub(super) const PROJECT_COLUMNS: &str = "id, name, path, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at";
pub(super) const CHAT_COLUMNS: &str = "id, project_id, name, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at";

#[must_use]
pub struct InfrastructureTransaction {
    database_transaction: Option<Transaction<'static, Sqlite>>,
    operation_failed: bool,
}

impl InfrastructureTransaction {
    pub(crate) async fn begin(connection_pool: &SqlitePool) -> InfrastructureResult<Self> {
        let database_transaction = connection_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|source| {
                database_operation_error("failed to begin database transaction", source)
            })?;

        Ok(Self {
            database_transaction: Some(database_transaction),
            operation_failed: false,
        })
    }

    pub async fn commit(mut self) -> InfrastructureResult<()> {
        let database_transaction = self.take_database_transaction()?;

        if self.operation_failed {
            database_transaction.rollback().await.map_err(|source| {
                database_operation_error("failed to roll back failed transaction", source)
            })?;
            return Err(InfrastructureError::new(
                InfrastructureErrorKind::TransactionWasMarkedAsFailed,
                "database transaction contains a failed operation",
            ));
        }

        database_transaction
            .commit()
            .await
            .map_err(|source| database_operation_error("failed to commit transaction", source))
    }

    pub async fn rollback(mut self) -> InfrastructureResult<()> {
        self.take_database_transaction()?
            .rollback()
            .await
            .map_err(|source| database_operation_error("failed to roll back transaction", source))
    }

    pub fn sql_builder(&mut self) -> SqlBuilderFactory<'_> {
        SqlBuilderFactory::for_transaction(self)
    }
    pub(crate) fn database_transaction_mut(
        &mut self,
    ) -> InfrastructureResult<&mut Transaction<'static, Sqlite>> {
        self.database_transaction.as_mut().ok_or_else(|| {
            InfrastructureError::new(
                InfrastructureErrorKind::DatabaseOperationFailed,
                "database transaction has already finished",
            )
        })
    }

    pub(crate) fn mark_operation_as_failed(&mut self) {
        self.operation_failed = true;
    }

    fn take_database_transaction(&mut self) -> InfrastructureResult<Transaction<'static, Sqlite>> {
        self.database_transaction.take().ok_or_else(|| {
            InfrastructureError::new(
                InfrastructureErrorKind::DatabaseOperationFailed,
                "database transaction has already finished",
            )
        })
    }

    fn record_operation_result<ReturnValue>(
        &mut self,
        operation_result: InfrastructureResult<ReturnValue>,
    ) -> InfrastructureResult<ReturnValue> {
        if operation_result.is_err() {
            self.operation_failed = true;
        }

        operation_result
    }

    async fn set_boolean_project_value(
        &mut self,
        column_name: &str,
        column_value: bool,
        project_id: ProjectId,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_boolean_entity_value(
            self.database_transaction_mut()?,
            "projects",
            column_name,
            column_value,
            project_id.as_bytes().to_vec(),
        )
        .await;

        self.record_operation_result(operation_result)
    }

    async fn set_usage_project_value(
        &mut self,
        column_name: &str,
        column_value: u8,
        project_id: ProjectId,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_usage_entity_value(
            self.database_transaction_mut()?,
            "projects",
            column_name,
            column_value,
            project_id.as_bytes().to_vec(),
        )
        .await;

        self.record_operation_result(operation_result)
    }

    async fn set_boolean_chat_value(
        &mut self,
        column_name: &str,
        column_value: bool,
        chat_id: ChatId,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_boolean_entity_value(
            self.database_transaction_mut()?,
            "chats",
            column_name,
            column_value,
            chat_id.as_bytes().to_vec(),
        )
        .await;

        self.record_operation_result(operation_result)
    }

    async fn set_usage_chat_value(
        &mut self,
        column_name: &str,
        column_value: u8,
        chat_id: ChatId,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_usage_entity_value(
            self.database_transaction_mut()?,
            "chats",
            column_name,
            column_value,
            chat_id.as_bytes().to_vec(),
        )
        .await;

        self.record_operation_result(operation_result)
    }
}
