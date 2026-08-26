use crate::database::{
    clone_database_file_path, create_database_connection_pool, database_operation_error,
};
use crate::mapping::map_project_row;
use crate::{
    DatabaseSchemaVersion, InfrastructureResult, InfrastructureTransaction, Project, ProjectId,
    SqlBuilderFactory,
};
use sqlx::{AssertSqlSafe, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Infrastructure {
    database_file_path: Arc<PathBuf>,
    pub(crate) connection_pool: SqlitePool,
}

impl Infrastructure {
    pub async fn init(database_file_path: impl AsRef<Path>) -> InfrastructureResult<Self> {
        let database_file_path = clone_database_file_path(database_file_path.as_ref());
        let connection_pool = create_database_connection_pool(&database_file_path).await?;

        Ok(Self {
            database_file_path: Arc::new(database_file_path),
            connection_pool,
        })
    }

    pub fn database_file_path(&self) -> &Path {
        self.database_file_path.as_path()
    }

    pub async fn schema_version(&self) -> InfrastructureResult<DatabaseSchemaVersion> {
        let schema_version = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1",
        )
        .fetch_one(&self.connection_pool)
        .await
        .map_err(|source| database_operation_error("failed to read schema version", source))?
        .unwrap_or(0);

        Ok(DatabaseSchemaVersion(schema_version))
    }

    pub async fn make_transaction(&self) -> InfrastructureResult<InfrastructureTransaction> {
        InfrastructureTransaction::begin(&self.connection_pool).await
    }

    pub async fn execute_db_actions<ReturnValue>(
        &self,
        database_actions: impl for<'transaction> std::ops::AsyncFnOnce(
            &'transaction mut InfrastructureTransaction,
        ) -> InfrastructureResult<
            ReturnValue,
        >,
    ) -> InfrastructureResult<ReturnValue> {
        let mut transaction = self.make_transaction().await?;
        match database_actions(&mut transaction).await {
            Ok(return_value) => {
                transaction.commit().await?;
                Ok(return_value)
            }
            Err(database_action_error) => match transaction.rollback().await {
                Ok(()) => Err(database_action_error),
                Err(rollback_error) => Err(crate::InfrastructureError::new(
                    crate::InfrastructureErrorKind::DatabaseActionAndRollbackFailed,
                    format!(
                        "database action failed: {database_action_error}; rollback failed: {rollback_error}"
                    ),
                )),
            },
        }
    }

    pub fn sql_builder(&self) -> SqlBuilderFactory<'_> {
        SqlBuilderFactory::for_infrastructure(self)
    }

    pub async fn get_project_by_id(
        &self,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<Option<Project>> {
        self.get_project_by_column("id", project_id.into().as_bytes().to_vec())
            .await
    }

    pub async fn get_project_by_name(
        &self,
        project_name: &str,
    ) -> InfrastructureResult<Option<Project>> {
        self.get_project_by_column("name", project_name.to_owned())
            .await
    }

    pub async fn get_project_by_path(
        &self,
        project_path: &str,
    ) -> InfrastructureResult<Option<Project>> {
        self.get_project_by_column("path", project_path.to_owned())
            .await
    }

    async fn get_project_by_column<ColumnValue>(
        &self,
        column_name: &str,
        column_value: ColumnValue,
    ) -> InfrastructureResult<Option<Project>>
    where
        ColumnValue:
            Send + for<'query> sqlx::Encode<'query, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
    {
        let query_text = format!(
            "SELECT id, name, path, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at FROM projects WHERE {column_name} = ?"
        );
        let database_row = sqlx::query(AssertSqlSafe(query_text))
            .bind(column_value)
            .fetch_optional(&self.connection_pool)
            .await
            .map_err(|source| database_operation_error("failed to get project", source))?;
        database_row.as_ref().map(map_project_row).transpose()
    }

    pub async fn close(&self) -> InfrastructureResult<()> {
        self.connection_pool.close().await;
        Ok(())
    }
}
