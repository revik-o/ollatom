use super::*;

#[tokio::test]
async fn rejects_configuration_keys_that_conflict_with_existing_values() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;
    let handle = configuration_store
        .clone()
        .create_update()
        .add_parameter("app", "ollatom")
        .unwrap()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap();

    assert!(matches!(
        handle.wait().await,
        ConfigurationCommitStatus::Failed(message) if message.contains("conflicts")
    ));
    assert_eq!(configuration_store.read_parameter("app").await.unwrap(), None);
}

#[tokio::test]
async fn concurrent_conflicting_json_commits_leave_no_partial_update() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;
    let first_handle = configuration_store
        .clone()
        .add_parameter("app", "ollatom")
        .unwrap()
        .commit()
        .unwrap();
    let second_handle = configuration_store
        .clone()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap();

    let (first_status, second_status) = tokio::join!(first_handle.wait(), second_handle.wait());
    assert_ne!(
        first_status == ConfigurationCommitStatus::Succeeded,
        second_status == ConfigurationCommitStatus::Succeeded
    );
    let app_value = configuration_store.read_parameter("app").await.unwrap();
    let language_value = configuration_store.read_parameter("app.language").await.unwrap();
    assert!(!(app_value.is_some() && language_value.is_some()));
}

#[tokio::test]
async fn commit_handle_reports_completion_after_wait_future_is_dropped() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;
    let status_store = configuration_store.clone();
    let handle = configuration_store
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap();
    let operation_identifier = handle.operation_identifier();
    assert!(operation_identifier.is_valid());
    assert_eq!(handle.status(), ConfigurationCommitStatus::Pending);

    drop(handle.wait());
    let reopened_store = open_json_store(temporary_directory.path()).await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if reopened_store.read_parameter("app.language").await.unwrap().is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        status_store.commit_status(operation_identifier),
        Some(ConfigurationCommitStatus::Succeeded)
    );
    assert_eq!(
        status_store.acknowledge_commit(operation_identifier),
        Some(ConfigurationCommitStatus::Succeeded)
    );
    assert_eq!(status_store.commit_status(operation_identifier), None);
}

#[tokio::test]
async fn rejects_invalid_dotted_keys_before_starting_a_commit() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;

    assert!(matches!(
        configuration_store.add_parameter("app..language", "en"),
        Err(YamlConfigurationError::InvalidYamlConfigurationKey { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn hardens_existing_configuration_lock_permissions() {
    let temporary_directory = tempdir().unwrap();
    let lock_file_path = temporary_directory.path().join("application.yaml.lock");
    std::fs::write(&lock_file_path, []).unwrap();
    std::fs::set_permissions(&lock_file_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;

    let commit_status = configuration_store
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;

    assert_eq!(commit_status, ConfigurationCommitStatus::Succeeded);
    let lock_file_mode = std::fs::metadata(lock_file_path).unwrap().mode() & 0o777;
    assert_eq!(lock_file_mode, 0o600);
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_symbolic_links_as_configuration_lock_files() {
    let temporary_directory = tempdir().unwrap();
    let lock_target_path = temporary_directory.path().join("lock-target");
    let lock_file_path = temporary_directory.path().join("application.yaml.lock");
    std::fs::write(&lock_target_path, "unchanged").unwrap();
    std::os::unix::fs::symlink(&lock_target_path, &lock_file_path).unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;

    let commit_status = configuration_store
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;

    assert!(matches!(
        commit_status,
        ConfigurationCommitStatus::Failed(message) if message.contains("symbolic link") || message.contains("Too many levels")
    ));
    assert!(
        std::fs::symlink_metadata(lock_file_path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(lock_target_path).unwrap(), "unchanged");
}

#[cfg(unix)]
#[tokio::test]
async fn hardens_yaml_directory_before_returning_a_store() {
    let temporary_directory = tempdir().unwrap();
    std::fs::set_permissions(temporary_directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

    let configuration_store = open_json_store(temporary_directory.path()).await;

    assert_eq!(
        std::fs::metadata(temporary_directory.path()).unwrap().mode() & 0o777,
        0o700
    );
    assert!(configuration_store.file_path().is_file());
}
