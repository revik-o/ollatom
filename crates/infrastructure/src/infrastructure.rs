use crate::database::{
    clone_database_file_path, create_database_connection_pool, database_operation_error,
};
use crate::mapping::map_project_row;
use crate::{
    DatabaseConfiguration, DatabaseSchemaVersion, InfrastructureResult, InfrastructureTransaction,
    Project, ProjectId,
};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Infrastructure {
    database_file_path: Arc<PathBuf>,
    pub(crate) connection_pool: SqlitePool,
}

impl Infrastructure {
    pub async fn init(database_file_path: impl AsRef<Path>) -> InfrastructureResult<Self> {
        Self::init_with_configuration(database_file_path, DatabaseConfiguration::default()).await
    }

    pub async fn init_with_configuration(
        database_file_path: impl AsRef<Path>,
        database_configuration: DatabaseConfiguration,
    ) -> InfrastructureResult<Self> {
        let database_file_path = clone_database_file_path(database_file_path.as_ref());
        let connection_pool =
            create_database_connection_pool(&database_file_path, database_configuration).await?;

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
        self.begin_write_transaction().await
    }

    pub async fn begin_read_transaction(&self) -> InfrastructureResult<InfrastructureTransaction> {
        InfrastructureTransaction::begin_deferred(&self.connection_pool).await
    }

    pub async fn begin_write_transaction(&self) -> InfrastructureResult<InfrastructureTransaction> {
        InfrastructureTransaction::begin_immediate(&self.connection_pool).await
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

    pub async fn get_project_by_id(
        &self,
        project_id: impl Into<ProjectId>,
    ) -> InfrastructureResult<Option<Project>> {
        self.get_project_by_lookup(ProjectLookup::Identifier(project_id.into()))
            .await
    }

    pub async fn get_project_by_name(
        &self,
        project_name: &str,
    ) -> InfrastructureResult<Option<Project>> {
        self.get_project_by_lookup(ProjectLookup::Name(project_name))
            .await
    }

    pub async fn get_project_by_path(
        &self,
        project_path: impl AsRef<Path>,
    ) -> InfrastructureResult<Option<Project>> {
        let mut transaction = self.begin_read_transaction().await?;
        let project = transaction.get_project_by_path(project_path).await?;
        transaction.commit().await?;
        Ok(project)
    }

    async fn get_project_by_lookup(
        &self,
        project_lookup: ProjectLookup<'_>,
    ) -> InfrastructureResult<Option<Project>> {
        let database_row = match project_lookup {
            ProjectLookup::Identifier(project_identifier) => {
                sqlx::query("SELECT id, name, canonical_directory_path, filesystem_identity_kind, filesystem_volume_identifier, filesystem_object_identifier, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at FROM projects WHERE id = ?")
                    .bind(project_identifier.as_bytes().to_vec())
                    .fetch_optional(&self.connection_pool)
                    .await
            }
            ProjectLookup::Name(project_name) => {
                sqlx::query("SELECT id, name, canonical_directory_path, filesystem_identity_kind, filesystem_volume_identifier, filesystem_object_identifier, llm_thinking_enabled, llm_context_optimization_enabled, cpu_usage_percentage, gpu_usage_percentage, created_at, updated_at FROM projects WHERE name = ?")
                    .bind(project_name)
                    .fetch_optional(&self.connection_pool)
                    .await
            }
        }
        .map_err(|source| database_operation_error("failed to get project", source))?;
        database_row.as_ref().map(map_project_row).transpose()
    }
}

enum ProjectLookup<'a> {
    Identifier(ProjectId),
    Name(&'a str),
}
