mod support;

use infrastructure::{Infrastructure, InfrastructureErrorKind, ProjectInitializationParameters};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use tempfile::{TempDir, tempdir};

#[tokio::test]
async fn initializes_database_at_explicit_absolute_file_path() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();

    assert_eq!(infrastructure.database_file_path(), database_file_path);
    assert_eq!(infrastructure.schema_version().await.unwrap().0, 1);
    assert!(database_file_path.is_file());

    drop(infrastructure);
}

#[tokio::test]
async fn initializes_database_when_filename_contains_url_delimiters() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("database?name#x.sqlite3");
    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();

    assert_eq!(infrastructure.database_file_path(), database_file_path);
    assert!(database_file_path.is_file());
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
    let temporary_directory = create_private_temporary_directory();
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
    let temporary_directory = create_private_temporary_directory();
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
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let first_infrastructure = Infrastructure::init(&database_file_path).await.unwrap();
    drop(first_infrastructure);

    let reopened_infrastructure = Infrastructure::init(database_file_path).await.unwrap();

    assert_eq!(reopened_infrastructure.schema_version().await.unwrap().0, 1);
    drop(reopened_infrastructure);
}

#[tokio::test]
async fn data_survives_closing_and_reopening_the_database() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("ollatom.sqlite3");
    let project_directory_path = temporary_directory.path().join("persisted-project");
    fs::create_dir(&project_directory_path).unwrap();
    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();
    let project = infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Persisted project",
                    &project_directory_path,
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    drop(infrastructure);

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
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("invalid.sqlite3");
    let original_contents = b"not a SQLite database";
    fs::write(&database_file_path, original_contents).unwrap();

    assert!(Infrastructure::init(&database_file_path).await.is_err());
    assert_eq!(fs::read(&database_file_path).unwrap(), original_contents);
}

#[cfg(unix)]
#[tokio::test]
async fn creates_database_file_with_owner_only_permissions() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("private.sqlite3");

    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();
    let database_mode = fs::metadata(&database_file_path).unwrap().mode() & 0o777;
    let directory_mode = fs::metadata(temporary_directory.path()).unwrap().mode() & 0o777;

    assert_eq!(database_mode, 0o600);
    assert_eq!(directory_mode & 0o077, 0);
    drop(infrastructure);
}

#[cfg(unix)]
#[tokio::test]
async fn hardens_database_directory_before_creating_database_files() {
    let temporary_directory = create_private_temporary_directory();
    let database_directory_path = temporary_directory.path().join("insecure");
    fs::create_dir(&database_directory_path).unwrap();
    fs::set_permissions(&database_directory_path, fs::Permissions::from_mode(0o755)).unwrap();
    let database_file_path = database_directory_path.join("database.sqlite3");

    let infrastructure = Infrastructure::init(&database_file_path).await.unwrap();

    assert_eq!(
        fs::metadata(&database_directory_path).unwrap().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&database_file_path).unwrap().mode() & 0o777,
        0o600
    );
    drop(infrastructure);
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_database_symlinks_without_modifying_the_target() {
    let temporary_directory = create_private_temporary_directory();
    let target_file_path = temporary_directory.path().join("target.sqlite3");
    let database_symlink_path = temporary_directory.path().join("database.sqlite3");
    let original_contents = b"preserve this target";
    fs::write(&target_file_path, original_contents).unwrap();
    symlink(&target_file_path, &database_symlink_path).unwrap();

    let initialization_error = Infrastructure::init(&database_symlink_path)
        .await
        .unwrap_err();

    assert_eq!(
        initialization_error.kind(),
        InfrastructureErrorKind::DatabaseConfigurationFailed
    );
    assert_eq!(fs::read(&target_file_path).unwrap(), original_contents);
}

#[cfg(not(unix))]
#[tokio::test]
async fn rejects_database_initialization_without_private_acl_support() {
    let temporary_directory = create_private_temporary_directory();
    let database_file_path = temporary_directory.path().join("database.sqlite3");

    let initialization_error = Infrastructure::init(&database_file_path).await.unwrap_err();

    assert_eq!(
        initialization_error.kind(),
        InfrastructureErrorKind::DatabaseConfigurationFailed
    );
    assert!(!database_file_path.exists());
}

fn create_private_temporary_directory() -> TempDir {
    let temporary_directory = tempdir().unwrap();
    #[cfg(unix)]
    fs::set_permissions(
        temporary_directory.path(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    temporary_directory
}
