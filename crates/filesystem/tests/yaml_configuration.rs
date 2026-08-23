use filesystem::{FilesystemError, create_yaml_configuration_file};
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
    let first_configuration_file = configuration_file
        .clone()
        .add_parameter("app.language", "en")
        .unwrap();
    let second_configuration_file = configuration_file
        .clone()
        .add_parameter("app.window.width", 1280)
        .unwrap();

    let (first_result, second_result) = tokio::join!(
        first_configuration_file.commit(),
        second_configuration_file.commit()
    );
    first_result.unwrap();
    second_result.unwrap();

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
    let result = configuration_store
        .create_update()
        .add_parameter("app", "ollatom")
        .unwrap()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .await;

    assert!(matches!(
        result,
        Err(FilesystemError::YamlConfigurationKeyConflict { .. })
    ));
    assert_eq!(
        configuration_store.read_parameter("app").await.unwrap(),
        None
    );
}
