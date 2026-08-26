mod support;

use infrastructure::{
    Infrastructure, InfrastructureErrorKind, ProjectInitializationParameters, SqlValue,
};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn initializes_database_at_explicit_absolute_file_path() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();

    assert_eq!(infrastructure.database_file_path(), database_file_path);
    assert_eq!(infrastructure.schema_version().await.unwrap().0, 1);
    assert!(database_file_path.is_file());

    infrastructure.close().await.unwrap();
}

#[tokio::test]
async fn rejects_relative_database_file_path() {
    let initialization_error = Infrastructure::init("ollatom.sqlite3").await.unwrap_err();

    assert_eq!(
        initialization_error.kind(),
        InfrastructureErrorKind::InvalidDatabaseFilePath
    );
}

#[tokio::test]
async fn rejects_missing_database_parent_directory() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory
        .path()
        .join("missing")
        .join("ollatom.sqlite3");
    let initialization_error = Infrastructure::init(database_file_path).await.unwrap_err();

    assert_eq!(
        initialization_error.kind(),
        InfrastructureErrorKind::DatabaseParentDirectoryDoesNotExist
    );
}

#[tokio::test]
async fn rejects_directory_database_file_path() {
    let temporary_directory = tempdir().unwrap();
    let initialization_error = Infrastructure::init(temporary_directory.path())
        .await
        .unwrap_err();

    assert_eq!(
        initialization_error.kind(),
        InfrastructureErrorKind::DatabasePathPointsToDirectory
    );
}

#[tokio::test]
async fn reopens_existing_database_without_reapplying_migrations() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let first_infrastructure = Infrastructure::init(&database_file_path).await.unwrap();
    first_infrastructure.close().await.unwrap();

    let reopened_infrastructure = Infrastructure::init(database_file_path).await.unwrap();

    assert_eq!(reopened_infrastructure.schema_version().await.unwrap().0, 1);
    reopened_infrastructure.close().await.unwrap();
}

#[tokio::test]
async fn data_survives_closing_and_reopening_the_database() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();
    let project = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Persisted project",
                    "/projects/persisted",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    infrastructure.close().await.unwrap();

    let reopened_infrastructure = Infrastructure::init(database_file_path).await.unwrap();
    assert_eq!(
        reopened_infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Persisted project"
    );
}

#[tokio::test]
async fn failed_initialization_preserves_existing_database_file() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("invalid.sqlite3");
    let original_contents = b"not a SQLite database";
    fs::write(&database_file_path, original_contents).unwrap();

    assert!(Infrastructure::init(&database_file_path).await.is_err());
    assert_eq!(fs::read(&database_file_path).unwrap(), original_contents);
}

#[tokio::test]
async fn configured_pragmas_are_available_through_sqlite_pragma_tables() {
    let temporary_directory = tempdir().unwrap();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(database_file_path).await.unwrap();

    let journal_mode_rows = infrastructure
        .sql_builder()
        .select(["journal_mode"])
        .from("pragma_journal_mode")
        .commit()
        .await
        .unwrap();
    let foreign_key_rows = infrastructure
        .sql_builder()
        .select(["foreign_keys"])
        .from("pragma_foreign_keys")
        .commit()
        .await
        .unwrap();
    let synchronous_rows = infrastructure
        .sql_builder()
        .select(["synchronous"])
        .from("pragma_synchronous")
        .commit()
        .await
        .unwrap();
    let busy_timeout_rows = infrastructure
        .sql_builder()
        .select(["timeout"])
        .from("pragma_busy_timeout")
        .commit()
        .await
        .unwrap();

    assert_eq!(
        journal_mode_rows[0]
            .try_get::<String>("journal_mode")
            .unwrap(),
        "wal"
    );
    assert_eq!(
        foreign_key_rows[0]
            .try_get::<SqlValue>("foreign_keys")
            .unwrap(),
        SqlValue::Integer(1)
    );
    assert_eq!(
        synchronous_rows[0]
            .try_get::<SqlValue>("synchronous")
            .unwrap(),
        SqlValue::Integer(1)
    );
    assert_eq!(
        busy_timeout_rows[0].try_get::<SqlValue>("timeout").unwrap(),
        SqlValue::Integer(5_000)
    );
}
