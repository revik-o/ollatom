use infrastructure::{YamlConfigurationError, create_yaml_configuration_file};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::tempdir;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct TestConfiguration {
    enabled: bool,
    name: String,
}

#[tokio::test]
async fn creates_and_reads_nested_configuration_parameters() {
    let temporary_directory = tempdir().unwrap();
    let configuration_file =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap()
            .add_parameter("app.language", "en")
            .unwrap()
            .add_parameter("app.window.width", 1280)
            .unwrap()
            .commit()
            .await
            .unwrap();

    assert_eq!(
        configuration_file
            .read_parameter("app.language")
            .await
            .unwrap(),
        Some(Value::String("en".to_owned()))
    );
    assert_eq!(
        configuration_file
            .read_parameter("app.window.width")
            .await
            .unwrap(),
        Some(Value::from(1280))
    );
}

#[tokio::test]
async fn opens_existing_configuration_without_removing_values() {
    let temporary_directory = tempdir().unwrap();
    create_yaml_configuration_file("application.yaml", temporary_directory.path())
        .await
        .unwrap()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .await
        .unwrap();

    let reopened_configuration_file =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap()
            .add_parameter("app.scale", 1.25)
            .unwrap()
            .commit()
            .await
            .unwrap();

    assert_eq!(
        reopened_configuration_file
            .read_parameter("app.language")
            .await
            .unwrap(),
        Some(Value::String("en".to_owned()))
    );
    assert_eq!(
        reopened_configuration_file
            .read_parameter("app.scale")
            .await
            .unwrap(),
        Some(Value::from(1.25))
    );
}

#[tokio::test]
async fn serializes_concurrent_configuration_commits() {
    let temporary_directory = tempdir().unwrap();
    let configuration_file =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap();
    let first_configuration_update = configuration_file
        .clone()
        .add_parameter("app.language", "en")
        .unwrap();
    let second_configuration_update = configuration_file
        .clone()
        .add_parameter("app.window.width", 1280)
        .unwrap();

    let (first_commit_result, second_commit_result) = tokio::join!(
        first_configuration_update.commit(),
        second_configuration_update.commit()
    );
    first_commit_result.unwrap();
    second_commit_result.unwrap();

    assert!(
        configuration_file
            .read_parameter("app.language")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        configuration_file
            .read_parameter("app.window.width")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn reads_and_writes_typed_yaml_configuration() {
    let temporary_directory = tempdir().unwrap();
    let configuration_file =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap();
    let expected_configuration = TestConfiguration {
        enabled: true,
        name: "ollatom".to_owned(),
    };

    configuration_file
        .write_yaml_configuration(&expected_configuration)
        .await
        .unwrap();
    let actual_configuration = configuration_file
        .read_yaml_configuration::<TestConfiguration>()
        .await
        .unwrap();

    assert_eq!(actual_configuration, expected_configuration);
}

#[tokio::test]
async fn rejects_configuration_keys_that_conflict_with_existing_values() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap();
    let commit_result = configuration_store
        .create_update()
        .add_parameter("app", "ollatom")
        .unwrap()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .await;

    assert!(matches!(
        commit_result,
        Err(YamlConfigurationError::YamlConfigurationKeyConflict { .. })
    ));
    assert_eq!(
        configuration_store.read_parameter("app").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn concurrent_conflicting_yaml_commits_leave_no_partial_update() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store =
        create_yaml_configuration_file("application.yaml", temporary_directory.path())
            .await
            .unwrap();
    let first_update = configuration_store
        .clone()
        .create_update()
        .add_parameter("app", "ollatom")
        .unwrap();
    let second_update = configuration_store
        .clone()
        .create_update()
        .add_parameter("app.language", "en")
        .unwrap();

    let (first_result, second_result) = tokio::join!(first_update.commit(), second_update.commit());
    assert_ne!(first_result.is_ok(), second_result.is_ok());
    let app_value = configuration_store.read_parameter("app").await.unwrap();
    let language_value = configuration_store
        .read_parameter("app.language")
        .await
        .unwrap();
    assert!(!(app_value.is_some() && language_value.is_some()));
}
