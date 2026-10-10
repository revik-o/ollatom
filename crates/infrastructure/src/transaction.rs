mod chat_api;
mod message_api;
mod project_api;
mod store;

use crate::database::database_operation_error;
use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use sqlx::{Sqlite, SqlitePool, Transaction};
use store::{
    BooleanEntitySetting, UsageEntitySetting, set_boolean_entity_value, set_usage_entity_value,
};

pub(super) const PROJECT_COLUMNS: &str = "id, name, canonical_directory_path, filesystem_identity_kind, filesystem_volume_identifier, filesystem_object_identifier, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at";
pub(super) const CHAT_COLUMNS: &str = "id, project_id, name, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at";

#[must_use]
pub struct InfrastructureTransaction {
    database_transaction: Option<Transaction<'static, Sqlite>>,
    operation_failed: bool,
}

impl InfrastructureTransaction {
    pub(crate) async fn begin_deferred(connection_pool: &SqlitePool) -> InfrastructureResult<Self> {
        Self::begin_with_mode(connection_pool, "BEGIN DEFERRED").await
    }

    pub(crate) async fn begin_immediate(
        connection_pool: &SqlitePool,
    ) -> InfrastructureResult<Self> {
        Self::begin_with_mode(connection_pool, "BEGIN IMMEDIATE").await
    }

    async fn begin_with_mode(
        connection_pool: &SqlitePool,
        begin_statement: &str,
    ) -> InfrastructureResult<Self> {
        let database_transaction = connection_pool
            .begin_with(match begin_statement {
                "BEGIN DEFERRED" => "BEGIN DEFERRED",
                "BEGIN IMMEDIATE" => "BEGIN IMMEDIATE",
                _ => unreachable!("transaction mode is fixed by the constructors"),
            })
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

    async fn set_boolean_setting(
        &mut self,
        setting: BooleanEntitySetting,
        column_value: bool,
        entity_id: Vec<u8>,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_boolean_entity_value(
            self.database_transaction_mut()?,
            setting,
            column_value,
            entity_id,
        )
        .await;

        self.record_operation_result(operation_result)
    }

    async fn set_usage_setting(
        &mut self,
        setting: UsageEntitySetting,
        column_value: u8,
        entity_id: Vec<u8>,
    ) -> InfrastructureResult<bool> {
        let operation_result = set_usage_entity_value(
            self.database_transaction_mut()?,
            setting,
            column_value,
            entity_id,
        )
        .await;

        self.record_operation_result(operation_result)
    }
}
