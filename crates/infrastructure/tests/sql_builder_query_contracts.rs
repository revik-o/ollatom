mod support;

use infrastructure::{
    InfrastructureErrorKind, ProjectInitializationParameters, SqlSortDirection, SqlValue,
};
use support::create_initialized_test_infrastructure;

#[tokio::test]
async fn supports_distinct_left_join_on_condition_grouping_ordering_and_paging() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project_without_chat = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Project without chat",
                    "/projects/without-chat",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let (project_with_chat, chat) =
        support::create_project_with_chat(&test_infrastructure.infrastructure).await;

    let joined_rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select_as("project.name", "project_name")
        .select_as("chat.name", "chat_name")
        .from_as("projects", "project")
        .left_join_as("chats", "chat")
        .on_condition(
            "project.id = chat.project_id AND chat.id = {}",
            [SqlValue::from(chat.id.as_uuid())],
        )
        .filter(
            "project.id = {} OR project.id = {}",
            [
                SqlValue::from(project_without_chat.id.as_uuid()),
                SqlValue::from(project_with_chat.id.as_uuid()),
            ],
        )
        .order_by("project.name", SqlSortDirection::Ascending)
        .limit(2)
        .offset(0)
        .commit()
        .await
        .unwrap();

    assert_eq!(joined_rows.len(), 2);
    assert_eq!(
        joined_rows[0].try_get::<String>("project_name").unwrap(),
        "Ollatom"
    );
    assert_eq!(
        joined_rows[1].try_get::<String>("project_name").unwrap(),
        "Project without chat"
    );
    assert_eq!(
        joined_rows[1].try_get::<SqlValue>("chat_name").unwrap(),
        SqlValue::Null
    );

    let grouped_rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("name AS project_name")
        .trusted_raw_expression("COUNT(*) AS project_count")
        .from("projects")
        .group_by("name")
        .having("COUNT(*) >= {}", [SqlValue::from(1_i64)])
        .order_by("name", SqlSortDirection::Descending)
        .limit(1)
        .offset(1)
        .distinct()
        .commit()
        .await
        .unwrap();

    assert_eq!(grouped_rows.len(), 1);
    assert_eq!(grouped_rows[0].try_get::<i64>("project_count").unwrap(), 1);

    let all_columns = test_infrastructure
        .infrastructure
        .sql_builder()
        .select_all("project")
        .from_as("projects", "project")
        .limit(1)
        .commit()
        .await
        .unwrap();
    assert!(all_columns[0].try_get::<String>("name").is_ok());
}

#[tokio::test]
async fn supports_filter_connectors_and_escaped_condition_braces() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    support::create_project_with_chat(&test_infrastructure.infrastructure).await;

    let rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["name"])
        .from("projects")
        .filter("name = {}", [SqlValue::from("Ollatom")])
        .and_filter("path = {}", [SqlValue::from("/projects/ollatom")])
        .or_filter("name = {}", [SqlValue::from("missing")])
        .filter(
            "name = {} AND '{{literal}}' = '{{literal}}'",
            [SqlValue::from("Ollatom")],
        )
        .commit()
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].try_get::<String>("name").unwrap(), "Ollatom");
}

#[tokio::test]
async fn preserves_duplicate_columns_by_index_and_rejects_ambiguous_names() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    support::create_project_with_chat(&test_infrastructure.infrastructure).await;

    let row = test_infrastructure
        .infrastructure
        .sql_builder()
        .select_as("project.name", "name")
        .select_as("chat.name", "name")
        .from_as("projects", "project")
        .inner_join_as("chats", "chat")
        .on("project.id", "chat.project_id")
        .commit()
        .await
        .unwrap()
        .pop()
        .unwrap();

    assert!(row.try_get::<String>("name").is_err());
    assert!(row.to_map().is_err());
    assert_eq!(row.try_get_index::<String>(0).unwrap(), "Ollatom");
    assert_eq!(row.try_get_index::<String>(1).unwrap(), "Initial chat");
}

#[tokio::test]
async fn supports_sql_value_and_typed_row_conversions() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    support::create_project_with_chat(&test_infrastructure.infrastructure).await;
    let row = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("NULL AS null_value")
        .trusted_raw_expression("1 AS integer_value")
        .trusted_raw_expression("CAST(1.5 AS REAL) AS real_value")
        .trusted_raw_expression("'text' AS text_value")
        .trusted_raw_expression("X'0102' AS blob_value")
        .trusted_raw_expression("'2026-08-25T00:00:00Z' AS timestamp_value")
        .trusted_raw_expression("'{\"enabled\":true}' AS json_value")
        .from("projects")
        .limit(1)
        .commit()
        .await
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(
        row.try_get::<SqlValue>("null_value").unwrap(),
        SqlValue::Null
    );
    assert_eq!(row.try_get::<i64>("integer_value").unwrap(), 1);
    assert_eq!(row.try_get::<f64>("real_value").unwrap(), 1.5);
    assert_eq!(row.try_get::<String>("text_value").unwrap(), "text");
    assert_eq!(row.try_get::<Vec<u8>>("blob_value").unwrap(), vec![1, 2]);
    assert_eq!(
        row.try_get::<time::OffsetDateTime>("timestamp_value")
            .unwrap()
            .year(),
        2026
    );
    assert_eq!(
        row.try_get::<serde_json::Value>("json_value").unwrap(),
        serde_json::json!({"enabled": true})
    );
    assert!(row.try_get::<bool>("integer_value").unwrap());
}

#[tokio::test]
async fn rejects_invalid_identifiers_fragments_and_row_conversion_failures() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let invalid_identifier = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["name;"])
        .from("projects")
        .commit()
        .await;
    assert_eq!(
        invalid_identifier.unwrap_err().kind(),
        InfrastructureErrorKind::InvalidSqlBuilderOperation
    );

    let unmatched_brace = test_infrastructure
        .infrastructure
        .sql_builder()
        .select(["name"])
        .from("projects")
        .filter("name = {", [SqlValue::from("Ollatom")])
        .commit()
        .await;
    assert!(unmatched_brace.is_err());

    let multiple_statements = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("name; DROP TABLE projects")
        .from("projects")
        .commit()
        .await;
    assert!(multiple_statements.is_err());

    support::create_project_with_chat(&test_infrastructure.infrastructure).await;
    let conversion_rows = test_infrastructure
        .infrastructure
        .sql_builder()
        .trusted_raw_expression("'not-a-timestamp' AS invalid_timestamp")
        .from("projects")
        .limit(1)
        .commit()
        .await;
    assert!(
        conversion_rows.unwrap()[0]
            .try_get::<time::OffsetDateTime>("invalid_timestamp")
            .is_err()
    );
}
