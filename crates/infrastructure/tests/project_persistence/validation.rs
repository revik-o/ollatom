use super::*;

#[tokio::test]
async fn project_entity_and_id_setters_validate_values_and_missing_targets() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_path = create_project_directory(&test_infrastructure, "setters");
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project("Setter project", &project_path, Default::default())
                .await
        })
        .await
        .unwrap();

    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            assert!(transaction.set_llm_thinking_for_project(true, &project).await?);
            assert!(transaction.set_llm_thinking_for_project(false, project.id).await?);
            assert!(
                transaction
                    .set_llm_context_optimization_for_project(true, &project)
                    .await?
            );
            assert!(
                transaction
                    .set_llm_context_optimization_for_project(false, project.id)
                    .await?
            );
            assert!(transaction.set_cpu_usage_for_project(75, &project).await?);
            assert!(transaction.set_cpu_usage_for_project(65, project.id).await?);
            assert!(transaction.set_gpu_usage_for_project(55, &project).await?);
            assert!(transaction.set_gpu_usage_for_project(45, project.id).await?);
            assert!(!transaction.set_cpu_usage_for_project(50, ProjectId::new()).await?);
            Ok(())
        })
        .await
        .unwrap();

    let invalid_cpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.set_cpu_usage_for_project(101, project.id).await)
        .await;
    assert!(invalid_cpu.is_err());
    let invalid_gpu = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| transaction.set_gpu_usage_for_project(101, project.id).await)
        .await;
    assert!(invalid_gpu.is_err());
}
