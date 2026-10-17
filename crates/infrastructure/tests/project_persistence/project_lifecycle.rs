use super::*;

#[tokio::test]
async fn creates_gets_updates_and_deletes_project() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_path = create_project_directory(&test_infrastructure, "project");
    let moved_project_path = test_infrastructure.temporary_directory.path().join("moved-project");
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Project", &project_path, ProjectInitializationParameters::default())
                .await
        })
        .await
        .unwrap();

    assert_eq!(
        test_infrastructure
            .infrastructure
            .get_project_by_name("Project")
            .await
            .unwrap()
            .unwrap()
            .id,
        project.id
    );

    std::fs::rename(&project_path, &moved_project_path).unwrap();
    let updated_project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_project(
                    ProjectUpdateOptions::new(project.id)
                        .with_name("Updated project")
                        .with_path(moved_project_path.clone())
                        .with_execution_settings(
                            ExecutionSettingsUpdate::default()
                                .with_large_language_model_thinking_enabled(true)
                                .with_context_optimization_enabled(true)
                                .with_cpu_usage_percentage(75)
                                .with_gpu_usage_percentage(65),
                        ),
                )
                .await
        })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated_project.name, "Updated project");
    assert_eq!(
        updated_project.canonical_directory_path,
        std::fs::canonicalize(&moved_project_path).unwrap()
    );
    assert_eq!(updated_project.filesystem_identity, project.filesystem_identity);
    assert!(updated_project.execution_settings.large_language_model_thinking_enabled);
    assert!(updated_project.execution_settings.context_optimization_enabled);
    assert_eq!(updated_project.execution_settings.cpu_usage_percentage, 75);
    assert_eq!(updated_project.execution_settings.gpu_usage_percentage, 65);

    let deleted_project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_project(&updated_project).await)
        .await
        .unwrap();

    assert_eq!(deleted_project.unwrap().id, project.id);
    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .is_none()
    );

    let id_deletion_path = create_project_directory(&test_infrastructure, "id-deletion");
    let project_for_id_deletion = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "ID deletion project",
                    &id_deletion_path,
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let deleted_by_id = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.delete_project(project_for_id_deletion.id).await)
        .await
        .unwrap();
    assert_eq!(deleted_by_id.unwrap().id, project_for_id_deletion.id);
}

#[tokio::test]
async fn project_settings_are_updated_independently() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_path = create_project_directory(&test_infrastructure, "settings");
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Settings project",
                    &project_path,
                    ProjectInitializationParameters::default(),
                )
                .await?;
            transaction.set_llm_thinking_for_project(true, &project).await?;
            transaction
                .set_llm_context_optimization_for_project(true, &project)
                .await?;
            transaction.set_cpu_usage_for_project(50, &project).await?;
            transaction.set_gpu_usage_for_project(25, &project).await?;
            transaction.get_project_by_id(project.id).await
        })
        .await
        .unwrap()
        .unwrap();

    assert!(project.execution_settings.large_language_model_thinking_enabled);
    assert!(project.execution_settings.context_optimization_enabled);
    assert_eq!(project.execution_settings.cpu_usage_percentage, 50);
    assert_eq!(project.execution_settings.gpu_usage_percentage, 25);
}
