use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use std::time::Duration;

mod pool;
mod security;
mod values;

pub(crate) use values::{
    clone_database_file_path, database_operation_error, format_timestamp, parse_optional_timestamp,
    parse_timestamp, parse_uuid, validate_nonblank_value, validate_usage_percentage,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DatabaseDurability {
    #[default]
    Durable,
    Balanced,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DatabaseConfiguration {
    pub durability: DatabaseDurability,
}
pub(crate) async fn create_database_connection_pool(
    database_file_path: &Path,
    database_configuration: DatabaseConfiguration,
) -> InfrastructureResult<SqlitePool> {
    security::validate_database_file_path(database_file_path)?;
    security::prepare_private_database_location(database_file_path)?;

    let connection_options = pool::create_sqlite_connection_options(
        database_file_path,
        database_configuration.durability,
    )?;

    let connection_pool = SqlitePoolOptions::new()
        .min_connections(1)
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(connection_options)
        .await
        .map_err(|source| {
            InfrastructureError::from_database_error(
                InfrastructureErrorKind::DatabaseConnectionFailed,
                "failed to open SQLite database",
                source,
            )
        })?;

    if let Err(error) = pool::verify_sqlite_connection_configuration(
        &connection_pool,
        database_configuration.durability,
    )
    .await
    {
        connection_pool.close().await;
        return Err(error);
    }

    if let Err(source) = sqlx::migrate!("./migrations").run(&connection_pool).await {
        connection_pool.close().await;

        return Err(InfrastructureError::from_database_error(
            InfrastructureErrorKind::DatabaseMigrationFailed,
            "failed to run database migrations",
            source,
        ));
    }

    Ok(connection_pool)
}
