use crate::database::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn create_private_temporary_directory() -> tempfile::TempDir {
    let temporary_directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(temporary_directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    temporary_directory
}

#[tokio::test]
async fn configures_durable_sqlite_connections() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("configuration.sqlite3");
    let connection_pool = create_database_connection_pool(&database_file_path, DatabaseConfiguration::default())
        .await
        .unwrap();

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&connection_pool)
        .await
        .unwrap();
    let synchronous_mode: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&connection_pool)
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");
    assert_eq!(synchronous_mode, 2);
}

#[tokio::test]
async fn configures_balanced_sqlite_connections_when_requested() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("configuration.sqlite3");
    let connection_pool = create_database_connection_pool(
        &database_file_path,
        DatabaseConfiguration {
            durability: DatabaseDurability::Balanced,
        },
    )
    .await
    .unwrap();

    let synchronous_mode: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&connection_pool)
        .await
        .unwrap();

    assert_eq!(synchronous_mode, 1);
}
