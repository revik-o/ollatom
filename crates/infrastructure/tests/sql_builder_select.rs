mod support;

use infrastructure::{ProjectInitializationParameters, SqlValue};
use support::create_initialized_test_infrastructure;

#[tokio::test]
async fn selects_joined_rows_with_bound_filter_values() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (project, chat) =
        support::create_project_with_chat(&test_infrastructure.infrastructure).await;

    let rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select_as("project.name", "project_name")
        .select_as("chat.name", "chat_name")
        .from_as("projects", "project")
        .inner_join_as("chats", "chat")
        .on("project.id", "chat.project_id")
        .filter(
            "project.id = {} AND chat.id = {}",
            [
                SqlValue::from(project.id.as_uuid()),
                SqlValue::from(chat.id.as_uuid()),
            ],
        )
        .commit()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].try_get::<String>("project_name").unwrap(),
        "Ollatom"
    );
    assert_eq!(
        rows[0].try_get::<String>("chat_name").unwrap(),
        "Initial chat"
    );
}

#[tokio::test]
async fn bound_value_cannot_change_sql_statement_structure() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Safe project",
                    "/projects/safe",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    let rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["id"])
        .from("projects")
        .filter("name = {}", [SqlValue::from("' OR 1 = 1 --")])
        .commit()
        .await
        .unwrap();

    assert!(rows.is_empty());
}

#[tokio::test]
async fn rejects_placeholder_count_mismatch() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let query_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["id"])
        .from("projects")
        .filter("name = {}", Vec::<SqlValue>::new())
        .commit()
        .await;

    assert!(query_result.is_err());
}

#[tokio::test]
async fn infrastructure_builder_rejects_transaction_only_terminals() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let fetch_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["id"])
        .from("projects")
        .fetch_all()
        .await;
    assert!(fetch_result.is_err());

    let execute_result = test_infrastructure
        .infrastructure
        .sql_builder()
        .update("projects")
        .set("name", "unused")
        .filter("id = {}", [SqlValue::from(uuid::Uuid::new_v4())])
        .execute()
        .await;
    assert!(execute_result.is_err());
}
