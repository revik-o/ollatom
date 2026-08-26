mod support;

use infrastructure::SqlValue;
use support::create_initialized_test_infrastructure;

#[tokio::test]
async fn inserts_updates_and_deletes_rows_with_explicit_commit() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_identifier = uuid::Uuid::new_v4();
    let timestamp = "2026-08-25T00:00:00Z";
    let insert_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("projects")
        .columns([
            "id",
            "name",
            "path",
            "llm_thinking_enabled",
            "llm_context_optimization_enabled",
            "cpu_usage_percentage",
            "gpu_usage_percentage",
            "created_at",
            "updated_at",
        ])
        .values([
            SqlValue::from(project_identifier),
            SqlValue::from("Builder project"),
            SqlValue::from("/projects/builder"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .commit()
        .await
        .unwrap();
    assert_eq!(insert_result.rows_affected, 1);

    let update_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .update("projects")
        .set("name", "Updated builder project")
        .filter("id = {}", [SqlValue::from(project_identifier)])
        .commit()
        .await
        .unwrap();
    assert_eq!(update_result.rows_affected, 1);

    let delete_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .delete_from("projects")
        .filter("id = {}", [SqlValue::from(project_identifier)])
        .commit()
        .await
        .unwrap();
    assert_eq!(delete_result.rows_affected, 1);
}

#[tokio::test]
async fn rejects_unfiltered_update_and_delete() {
    let test_infrastructure = create_initialized_test_infrastructure().await;

    let update_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .update("projects")
        .set("name", "Unsafe update")
        .commit()
        .await;
    let delete_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .delete_from("projects")
        .commit()
        .await;

    assert!(update_result.is_err());
    assert!(delete_result.is_err());
}

#[tokio::test]
async fn inserts_multiple_rows_and_returns_selected_columns() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let first_project_identifier = uuid::Uuid::new_v4();
    let second_project_identifier = uuid::Uuid::new_v4();
    let timestamp = "2026-08-25T00:00:00Z";
    let insert_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("projects")
        .columns([
            "id",
            "name",
            "path",
            "llm_thinking_enabled",
            "llm_context_optimization_enabled",
            "cpu_usage_percentage",
            "gpu_usage_percentage",
            "created_at",
            "updated_at",
        ])
        .values([
            SqlValue::from(first_project_identifier),
            SqlValue::from("First builder project"),
            SqlValue::from("/projects/first-builder"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .values([
            SqlValue::from(second_project_identifier),
            SqlValue::from("Second builder project"),
            SqlValue::from("/projects/second-builder"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .returning(["id", "name"])
        .commit()
        .await
        .unwrap();

    assert_eq!(insert_result.rows_affected, 2);
    assert_eq!(insert_result.returned_rows.len(), 2);
}

#[tokio::test]
async fn permits_explicit_all_row_mutation() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let timestamp = "2026-08-25T00:00:00Z";
    test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("projects")
        .columns([
            "id",
            "name",
            "path",
            "llm_thinking_enabled",
            "llm_context_optimization_enabled",
            "cpu_usage_percentage",
            "gpu_usage_percentage",
            "created_at",
            "updated_at",
        ])
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from("All rows project"),
            SqlValue::from("/projects/all-rows"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .commit()
        .await
        .unwrap();

    let update_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .update("projects")
        .set("cpu_usage_percentage", 50_i64)
        .allow_all_rows()
        .commit()
        .await
        .unwrap();

    assert_eq!(update_result.rows_affected, 1);
}

#[tokio::test]
async fn update_and_delete_returning_rows_are_available() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Returning project",
                    "/projects/returning",
                    infrastructure::ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();

    let updated = test_infrastructure
        .infrastructure
        .sql_builder()
        .update("projects")
        .set("name", "Returned project")
        .filter("id = {}", [SqlValue::from(project.id.as_uuid())])
        .returning(["id", "name"])
        .commit()
        .await
        .unwrap();
    assert_eq!(updated.returned_rows.len(), 1);
    assert_eq!(
        updated.returned_rows[0].try_get::<String>("name").unwrap(),
        "Returned project"
    );

    let deleted = test_infrastructure
        .infrastructure
        .sql_builder()
        .delete_from("projects")
        .filter("id = {}", [SqlValue::from(project.id.as_uuid())])
        .returning(["id", "name"])
        .commit()
        .await
        .unwrap();
    assert_eq!(deleted.returned_rows.len(), 1);
    assert_eq!(deleted.rows_affected, 1);
}

#[tokio::test]
async fn permits_explicit_all_row_delete() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let timestamp = "2026-08-25T00:00:00Z";
    for project_name in ["First delete project", "Second delete project"] {
        test_infrastructure
            .infrastructure
            .sql_builder()
            .insert_into("projects")
            .columns([
                "id",
                "name",
                "path",
                "llm_thinking_enabled",
                "llm_context_optimization_enabled",
                "cpu_usage_percentage",
                "gpu_usage_percentage",
                "created_at",
                "updated_at",
            ])
            .values([
                SqlValue::from(uuid::Uuid::new_v4()),
                SqlValue::from(project_name),
                SqlValue::from(format!("/projects/{}", project_name.replace(' ', "-"))),
                SqlValue::from(false),
                SqlValue::from(false),
                SqlValue::from(100_u8),
                SqlValue::from(100_u8),
                SqlValue::from(timestamp),
                SqlValue::from(timestamp),
            ])
            .commit()
            .await
            .unwrap();
    }

    let delete_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .delete_from("projects")
        .allow_all_rows()
        .commit()
        .await
        .unwrap();
    assert_eq!(delete_result.rows_affected, 2);
}

#[tokio::test]
async fn automatic_builder_rolls_back_a_failed_multi_row_insert() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let timestamp = "2026-08-25T00:00:00Z";
    let duplicate_name = "Duplicate automatic project";
    let insert_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .insert_into("projects")
        .columns([
            "id",
            "name",
            "path",
            "llm_thinking_enabled",
            "llm_context_optimization_enabled",
            "cpu_usage_percentage",
            "gpu_usage_percentage",
            "created_at",
            "updated_at",
        ])
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(duplicate_name),
            SqlValue::from("/projects/automatic-one"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .values([
            SqlValue::from(uuid::Uuid::new_v4()),
            SqlValue::from(duplicate_name),
            SqlValue::from("/projects/automatic-two"),
            SqlValue::from(false),
            SqlValue::from(false),
            SqlValue::from(100_u8),
            SqlValue::from(100_u8),
            SqlValue::from(timestamp),
            SqlValue::from(timestamp),
        ])
        .commit()
        .await;
    assert!(insert_result.is_err());
    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_name(duplicate_name)
            .await
            .unwrap()
            .is_none()
    );
}
