use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub(crate) async fn create_database_connection_pool(
    database_file_path: &Path,
) -> InfrastructureResult<SqlitePool> {
    validate_database_file_path(database_file_path)?;
    let connection_options = create_sqlite_connection_options(database_file_path)?;
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

    if let Err(error) = verify_sqlite_connection_configuration(&connection_pool).await {
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

fn validate_database_file_path(database_file_path: &Path) -> InfrastructureResult<()> {
    if database_file_path.as_os_str().is_empty() || !database_file_path.is_absolute() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::InvalidDatabaseFilePath,
            format!(
                "SQLite database path '{}' must be an absolute file path",
                database_file_path.display()
            ),
        ));
    }

    if database_file_path.is_dir() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabasePathPointsToDirectory,
            format!(
                "SQLite database path '{}' points to a directory",
                database_file_path.display()
            ),
        ));
    }

    let parent_directory_path = database_file_path.parent().ok_or_else(|| {
        InfrastructureError::new(
            InfrastructureErrorKind::InvalidDatabaseFilePath,
            format!(
                "SQLite database path '{}' has no parent directory",
                database_file_path.display()
            ),
        )
    })?;

    if !parent_directory_path.is_dir() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseParentDirectoryDoesNotExist,
            format!(
                "SQLite database parent directory '{}' does not exist",
                parent_directory_path.display()
            ),
        ));
    }

    Ok(())
}

fn create_sqlite_connection_options(
    database_file_path: &Path,
) -> InfrastructureResult<SqliteConnectOptions> {
    let database_url = format!("sqlite://{}", database_file_path.display());
    SqliteConnectOptions::from_str(&database_url)
        .map_err(|source| {
            InfrastructureError::from_database_error(
                InfrastructureErrorKind::InvalidDatabaseFilePath,
                "failed to convert the SQLite database path into connection options",
                source,
            )
        })
        .map(|connection_options| {
            connection_options
                .filename(database_file_path)
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(5))
        })
}

async fn verify_sqlite_connection_configuration(
    connection_pool: &SqlitePool,
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

    if !journal_mode.eq_ignore_ascii_case("wal")
        || foreign_keys_enabled != 1
        || synchronous_mode != 1
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

pub(crate) fn database_operation_error(
    operation: &str,
    source: sqlx::Error,
) -> InfrastructureError {
    let error_kind = match &source {
        sqlx::Error::Database(database_error)
            if database_error
                .code()
                .is_some_and(|error_code| error_code.starts_with("19") || error_code == "2067") =>
        {
            InfrastructureErrorKind::DatabaseConstraintViolation
        }
        _ => InfrastructureErrorKind::DatabaseOperationFailed,
    };
    InfrastructureError::from_database_error(error_kind, operation, source)
}

pub(crate) fn format_timestamp(timestamp: OffsetDateTime) -> InfrastructureResult<String> {
    timestamp.format(&Rfc3339).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to format UTC timestamp: {source}"),
        )
    })
}

pub(crate) fn parse_timestamp(timestamp: &str) -> InfrastructureResult<OffsetDateTime> {
    OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to parse stored UTC timestamp '{timestamp}': {source}"),
        )
    })
}

pub(crate) fn parse_optional_timestamp(
    timestamp: Option<String>,
) -> InfrastructureResult<Option<OffsetDateTime>> {
    timestamp.as_deref().map(parse_timestamp).transpose()
}

pub(crate) fn parse_uuid(identifier_bytes: Vec<u8>) -> InfrastructureResult<Uuid> {
    Uuid::from_slice(&identifier_bytes).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to decode stored UUID: {source}"),
        )
    })
}

pub(crate) fn validate_nonblank_value(
    field_name: &str,
    field_value: &str,
) -> InfrastructureResult<()> {
    if field_value.trim().is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            format!("{field_name} must not be blank"),
        ));
    }

    Ok(())
}

pub(crate) fn validate_usage_percentage(
    field_name: &str,
    usage_percentage: u8,
) -> InfrastructureResult<()> {
    if usage_percentage > 100 {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            format!("{field_name} must be between 0 and 100"),
        ));
    }

    Ok(())
}

pub(crate) fn clone_database_file_path(database_file_path: &Path) -> PathBuf {
    database_file_path.to_owned()
}
