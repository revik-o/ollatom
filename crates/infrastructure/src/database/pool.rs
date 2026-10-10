use super::DatabaseDurability;
use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use std::path::Path;
use std::time::Duration;

pub(super) fn create_sqlite_connection_options(
    database_file_path: &Path,
    durability: DatabaseDurability,
) -> InfrastructureResult<SqliteConnectOptions> {
    Ok(SqliteConnectOptions::new()
        .filename(database_file_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(sqlite_synchronous_mode(durability))
        .busy_timeout(Duration::from_secs(5)))
}

fn sqlite_synchronous_mode(durability: DatabaseDurability) -> SqliteSynchronous {
    match durability {
        DatabaseDurability::Durable => SqliteSynchronous::Full,
        DatabaseDurability::Balanced => SqliteSynchronous::Normal,
    }
}

pub(super) async fn verify_sqlite_connection_configuration(
    connection_pool: &SqlitePool,
    durability: DatabaseDurability,
) -> InfrastructureResult<()> {
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(connection_pool)
        .await
        .map_err(|source| configuration_error("failed to read journal mode", source))?;
    let foreign_keys_enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(connection_pool)
        .await
        .map_err(|source| configuration_error("failed to read foreign key mode", source))?;
    let synchronous_mode: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(connection_pool)
        .await
        .map_err(|source| configuration_error("failed to read synchronous mode", source))?;
    let busy_timeout_milliseconds: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(connection_pool)
        .await
        .map_err(|source| configuration_error("failed to read busy timeout", source))?;

    let expected_synchronous_mode = match durability {
        DatabaseDurability::Durable => 2,
        DatabaseDurability::Balanced => 1,
    };

    if !journal_mode.eq_ignore_ascii_case("wal")
        || foreign_keys_enabled != 1
        || synchronous_mode != expected_synchronous_mode
        || busy_timeout_milliseconds != 5_000
    {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseConfigurationFailed,
            format!(
                "unexpected SQLite configuration: journal_mode={journal_mode}, foreign_keys={foreign_keys_enabled}, synchronous={synchronous_mode}, busy_timeout={busy_timeout_milliseconds}"
            ),
        ));
    }

    Ok(())
}

fn configuration_error(operation: &str, source: sqlx::Error) -> InfrastructureError {
    InfrastructureError::from_database_error(
        InfrastructureErrorKind::DatabaseConfigurationFailed,
        operation,
        source,
    )
}
