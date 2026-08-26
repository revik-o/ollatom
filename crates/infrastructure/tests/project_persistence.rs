mod support;

use infrastructure::{ProjectId, ProjectInitializationParameters, ProjectUpdateOptions};
use support::create_initialized_test_infrastructure;

#[tokio::test]
async fn creates_gets_updates_and_deletes_project() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Project",
                    "/projects/project",
                    ProjectInitializationParameters::default(),
                )
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

    let updated_project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_project(
                    ProjectUpdateOptions::new(project.id)
                        .with_name("Updated project")
                        .with_path("/projects/updated")
                        .with_llm_thinking_enabled(true)
                        .with_llm_context_optimization_enabled(true)
                        .with_cpu_usage_percentage(75)
                        .with_gpu_usage_percentage(65),
                )
                .await
        })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated_project.name, "Updated project");
    assert_eq!(updated_project.path, "/projects/updated");
    assert!(updated_project.llm_thinking_enabled);
    assert!(updated_project.llm_context_optimization_enabled);
    assert_eq!(updated_project.cpu_usage_percentage, 75);
    assert_eq!(updated_project.gpu_usage_percentage, 65);

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

    let project_for_id_deletion = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "ID deletion project",
                    "/projects/id-deletion",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let deleted_by_id = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .delete_project_by_id(project_for_id_deletion.id)
                .await
        })
        .await
        .unwrap();
    assert_eq!(deleted_by_id.unwrap().id, project_for_id_deletion.id);
}

#[tokio::test]
async fn project_settings_are_updated_independently() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let project = transaction
                .create_project(
                    "Settings project",
                    "/projects/settings",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            transaction
                .set_llm_thinking_for_project(true, &project)
                .await?;
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

    assert!(project.llm_thinking_enabled);
    assert!(project.llm_context_optimization_enabled);
    assert_eq!(project.cpu_usage_percentage, 50);
    assert_eq!(project.gpu_usage_percentage, 25);
}

#[tokio::test]
async fn project_names_and_paths_are_case_sensitive_and_preserve_input() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (first_project, second_project) = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let first_project = transaction
                .create_project(
                    "Case",
                    "/Projects/Case",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            let second_project = transaction
                .create_project(
                    "case",
                    "/projects/case",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            Ok((first_project, second_project))
        })
        .await
        .unwrap();

    assert_eq!(
        test_infrastructure
            .infrastructure
            .get_project_by_path("/Projects/Case")
            .await
            .unwrap()
            .unwrap()
            .id,
        first_project.id
    );
    assert_eq!(second_project.name, "case");

    let duplicate_name = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Case",
                    "/projects/another",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await;
    assert!(duplicate_name.is_err());

    let duplicate_path = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Another",
                    "/Projects/Case",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await;
    assert!(duplicate_path.is_err());
}

#[tokio::test]
async fn empty_and_missing_project_updates_preserve_state() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Stable project",
                    "/projects/stable",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let unchanged_project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .update_project(ProjectUpdateOptions::new(project.id))
                .await
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged_project.updated_at, project.updated_at);
    assert!(
        test_infrastructure
            .infrastructure
            .execute_db_actions(async |transaction| {
                transaction
                    .update_project(ProjectUpdateOptions::new(ProjectId::new()))
                    .await
            })
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn project_entity_and_id_setters_validate_values_and_missing_targets() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Setter project",
                    "/projects/setter",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();

    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            assert!(
                transaction
                    .set_llm_thinking_for_project(true, &project)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_thinking_for_project_by_id(false, project.id)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_context_optimization_for_project(true, &project)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_context_optimization_for_project_by_id(false, project.id)
                    .await?
            );
            assert!(transaction.set_cpu_usage_for_project(75, &project).await?);
            assert!(
                transaction
                    .set_cpu_usage_for_project_by_id(65, project.id)
                    .await?
            );
            assert!(transaction.set_gpu_usage_for_project(55, &project).await?);
            assert!(
                transaction
                    .set_gpu_usage_for_project_by_id(45, project.id)
                    .await?
            );
            assert!(
                !transaction
                    .set_cpu_usage_for_project_by_id(50, ProjectId::new())
                    .await?
            );
            Ok(())
        })
        .await
        .unwrap();

    let invalid_cpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .set_cpu_usage_for_project_by_id(101, project.id)
                .await
        })
        .await;
    assert!(invalid_cpu.is_err());
    let invalid_gpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .set_gpu_usage_for_project_by_id(101, project.id)
                .await
        })
        .await;
    assert!(invalid_gpu.is_err());
}
