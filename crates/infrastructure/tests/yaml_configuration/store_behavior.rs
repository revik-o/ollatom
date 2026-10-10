use super::*;

#[tokio::test]
async fn creates_and_reads_nested_configuration_parameters() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;
    let commit_status = configuration_store
        .clone()
        .add_parameter("app.language", "en")
        .unwrap()
        .add_parameter("app.window.width", 1280)
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;

    assert_eq!(commit_status, ConfigurationCommitStatus::Succeeded);
    assert_eq!(
        configuration_store
            .read_parameter("app.language")
            .await
            .unwrap(),
        Some(Value::String("en".to_owned()))
    );
    assert_eq!(
        configuration_store
            .read_parameter("app.window.width")
            .await
            .unwrap(),
        Some(Value::from(1280))
    );
}

#[tokio::test]
async fn opens_existing_configuration_without_removing_values() {
    let temporary_directory = tempdir().unwrap();
    let first_store = open_json_store(temporary_directory.path()).await;
    let first_commit_status = first_store
        .clone()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;
    assert_eq!(first_commit_status, ConfigurationCommitStatus::Succeeded);

    let reopened_store = open_json_store(temporary_directory.path()).await;
    let second_commit_status = reopened_store
        .clone()
        .add_parameter("app.scale", 1.25)
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;
    assert_eq!(second_commit_status, ConfigurationCommitStatus::Succeeded);

    assert_eq!(
        reopened_store.read_parameter("app.language").await.unwrap(),
        Some(Value::String("en".to_owned()))
    );
    assert_eq!(
        reopened_store.read_parameter("app.scale").await.unwrap(),
        Some(Value::from(1.25))
    );
}

#[tokio::test]
async fn serializes_concurrent_configuration_commits() {
    let temporary_directory = tempdir().unwrap();
    let configuration_store = open_json_store(temporary_directory.path()).await;
    let first_handle = configuration_store
        .clone()
        .add_parameter("app.language", "en")
        .unwrap()
        .commit()
        .unwrap();
    let second_handle = configuration_store
        .clone()
        .add_parameter("app.window.width", 1280)
        .unwrap()
        .commit()
        .unwrap();

    let (first_status, second_status) = tokio::join!(first_handle.wait(), second_handle.wait());
    assert_eq!(first_status, ConfigurationCommitStatus::Succeeded);
    assert_eq!(second_status, ConfigurationCommitStatus::Succeeded);
    assert!(
        configuration_store
            .read_parameter("app.language")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        configuration_store
            .read_parameter("app.window.width")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn serializes_commits_from_independently_opened_stores() {
    let temporary_directory = tempdir().unwrap();
    let first_store = open_json_store(temporary_directory.path()).await;
    let second_store = open_json_store(temporary_directory.path()).await;

    let first_handle = first_store
        .clone()
        .add_parameter("database.host", "database.example")
        .unwrap()
        .commit()
        .unwrap();
    let second_handle = second_store
        .add_parameter("database.port", 5432)
        .unwrap()
        .commit()
        .unwrap();
    let (first_status, second_status) = tokio::join!(first_handle.wait(), second_handle.wait());
    assert_eq!(first_status, ConfigurationCommitStatus::Succeeded);
    assert_eq!(second_status, ConfigurationCommitStatus::Succeeded);

    assert_eq!(
        first_store.read_parameter("database.host").await.unwrap(),
        Some(Value::String("database.example".to_owned()))
    );
    assert_eq!(
        first_store.read_parameter("database.port").await.unwrap(),
        Some(Value::from(5432))
    );
}

#[tokio::test]
async fn reads_changes_written_through_an_independent_store() {
    let temporary_directory = tempdir().unwrap();
    let first_store = open_json_store(temporary_directory.path()).await;
    let second_store = open_json_store(temporary_directory.path()).await;

    let status = second_store
        .add_parameter("database.host", "database.example")
        .unwrap()
        .commit()
        .unwrap()
        .wait()
        .await;
    assert_eq!(status, ConfigurationCommitStatus::Succeeded);
    assert_eq!(
        first_store.read_parameter("database.host").await.unwrap(),
        Some(Value::String("database.example".to_owned()))
    );
}

#[tokio::test]
async fn reads_and_writes_typed_yaml_configuration() {
    let temporary_directory = tempdir().unwrap();
    let configuration_file =
        create_typed_yaml_file::<TestConfiguration>("application.yaml", temporary_directory.path())
            .await
            .unwrap();
    let expected_configuration = TestConfiguration {
        enabled: true,
        name: "ollatom".to_owned(),
    };

    configuration_file
        .write(&expected_configuration)
        .await
        .unwrap();
    let actual_configuration = configuration_file.read().await.unwrap();

    assert_eq!(actual_configuration, expected_configuration);
}

#[tokio::test]
async fn typed_yaml_supports_scalar_and_sequence_roots() {
    let temporary_directory = tempdir().unwrap();
    let scalar_file = create_typed_yaml_file::<u32>("scalar.yaml", temporary_directory.path())
        .await
        .unwrap();
    scalar_file.write(&42).await.unwrap();
    assert_eq!(scalar_file.read().await.unwrap(), 42);

    let sequence_file =
        create_typed_yaml_file::<Vec<u32>>("sequence.yaml", temporary_directory.path())
            .await
            .unwrap();
    let expected_sequence = vec![1, 2, 3];
    sequence_file.write(&expected_sequence).await.unwrap();
    assert_eq!(sequence_file.read().await.unwrap(), expected_sequence);
}

#[tokio::test]
async fn typed_yaml_preserves_complex_keys_and_non_finite_numbers() {
    let temporary_directory = tempdir().unwrap();
    let complex_key_file = create_typed_yaml_file::<BTreeMap<BTreeMap<String, u32>, String>>(
        "complex-key.yaml",
        temporary_directory.path(),
    )
    .await
    .unwrap();
    let expected_mapping = BTreeMap::from([(
        BTreeMap::from([("answer".to_owned(), 42)]),
        "pair".to_owned(),
    )]);
    complex_key_file.write(&expected_mapping).await.unwrap();
    assert_eq!(complex_key_file.read().await.unwrap(), expected_mapping);

    let non_finite_file =
        create_typed_yaml_file::<f64>("non-finite.yaml", temporary_directory.path())
            .await
            .unwrap();
    non_finite_file.write(&f64::NAN).await.unwrap();
    assert!(non_finite_file.read().await.unwrap().is_nan());
}
