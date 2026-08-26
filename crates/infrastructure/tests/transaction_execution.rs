mod support;

use infrastructure::{InfrastructureErrorKind, ProjectInitializationParameters, SqlValue};
use support::create_initialized_test_infrastructure;

#[tokio::test]
async fn manual_commit_persists_all_transaction_operations() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let project = transaction
        .create_project(
            "Committed project",
            "/projects/committed",
            ProjectInitializationParameters::default(),
        )
        .await
        .unwrap();

    transaction.commit().await.unwrap();

    assert_eq!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Committed project"
    );
}

#[tokio::test]
async fn explicit_rollback_discards_all_transaction_operations() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let project = transaction
        .create_project(
            "Rolled back project",
            "/projects/rolled-back",
            ProjectInitializationParameters::default(),
        )
        .await
        .unwrap();

    transaction.rollback().await.unwrap();

    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn dropping_unfinished_transaction_discards_all_transaction_operations() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = {
        let mut transaction = test_infrastructure
            .infrastructure
            .make_transaction()
            .await
            .unwrap();
        transaction
            .create_project(
                "Dropped transaction project",
                "/projects/dropped",
                ProjectInitializationParameters::default(),
            )
            .await
            .unwrap()
    };

    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn execute_db_actions_commits_successful_closure() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Automatic project",
                    "/projects/automatic",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();

    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn execute_db_actions_rolls_back_successful_operations_after_later_error() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let database_actions_result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Automatic rollback project",
                    "/projects/automatic-rollback",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            transaction
                .create_project(
                    "Automatic rollback project",
                    "/projects/automatic-rollback-duplicate",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await;

    assert!(database_actions_result.is_err());
    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_name("Automatic rollback project")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn transaction_bound_sql_builder_participates_in_rollback() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let project = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "Builder rollback project",
                    "/projects/builder-rollback",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();

    transaction
        .sql_builder()
        .update("projects")
        .set("name", "Uncommitted builder name")
        .filter("id = {}", [SqlValue::from(project.id.as_uuid())])
        .execute()
        .await
        .unwrap();
    transaction.rollback().await.unwrap();

    assert_eq!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Builder rollback project"
    );
}

#[tokio::test]
async fn failed_operation_prevents_manual_transaction_commit() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    transaction
        .create_project(
            "Duplicate project",
            "/projects/first",
            ProjectInitializationParameters::default(),
        )
        .await
        .unwrap();
    let duplicate_result = transaction
        .create_project(
            "Duplicate project",
            "/projects/second",
            ProjectInitializationParameters::default(),
        )
        .await;

    assert!(duplicate_result.is_err());
    let commit_error = transaction.commit().await.unwrap_err();
    assert_eq!(
        commit_error.kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_name("Duplicate project")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn sql_builder_validation_failure_prevents_manual_transaction_commit() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();

    let query_result = transaction
        .sql_builder()
        .select(["id"])
        .from("projects;")
        .fetch_all()
        .await;

    assert!(query_result.is_err());
    assert_eq!(
        transaction.commit().await.unwrap_err().kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
}

#[tokio::test]
async fn sql_builder_cardinality_failure_prevents_manual_transaction_commit() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();

    let query_result = transaction
        .sql_builder()
        .select(["id"])
        .from("projects")
        .fetch_one()
        .await;

    assert!(query_result.is_err());
    assert_eq!(
        transaction.commit().await.unwrap_err().kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
}

#[tokio::test]
async fn every_transaction_builder_failure_poison_rolls_back_prior_mutations() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let project = transaction
        .create_project(
            "Poisoned transaction",
            "/projects/poisoned",
            ProjectInitializationParameters::default(),
        )
        .await
        .unwrap();
    let failure = transaction
        .sql_builder()
        .select(["id"])
        .from("projects")
        .filter("id = {}", Vec::<SqlValue>::new())
        .fetch_all()
        .await;

    assert_eq!(
        failure.unwrap_err().kind(),
        InfrastructureErrorKind::InvalidSqlBuilderOperation
    );
    assert_eq!(
        transaction.commit().await.unwrap_err().kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
    assert!(
        test_infrastructure
            .infrastructure
            .get_project_by_id(project.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn transaction_builder_rejects_wrong_terminal_and_sql_execution_failures() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut wrong_terminal_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let wrong_terminal_result = wrong_terminal_transaction
        .sql_builder()
        .select(["id"])
        .from("projects")
        .commit()
        .await;
    assert!(wrong_terminal_result.is_err());
    assert_eq!(
        wrong_terminal_transaction
            .commit()
            .await
            .unwrap_err()
            .kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );

    let mut execution_failure_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let execution_failure = execution_failure_transaction
        .sql_builder()
        .select(["id"])
        .from("missing_table")
        .fetch_all()
        .await;
    assert!(execution_failure.is_err());
    assert_eq!(
        execution_failure_transaction
            .commit()
            .await
            .unwrap_err()
            .kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
}

#[tokio::test]
async fn transaction_builder_rejects_missing_components_and_optional_cardinality() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let mut missing_selection_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let missing_selection = missing_selection_transaction
        .sql_builder()
        .select(Vec::<&str>::new())
        .from("projects")
        .fetch_all()
        .await;
    assert!(missing_selection.is_err());
    assert!(missing_selection_transaction.commit().await.is_err());

    let mut missing_join_condition_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let missing_join_condition = missing_join_condition_transaction
        .sql_builder()
        .select(["projects.id"])
        .from("projects")
        .inner_join("chats")
        .fetch_all()
        .await;
    assert!(missing_join_condition.is_err());
    assert!(missing_join_condition_transaction.commit().await.is_err());

    test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .create_project(
                    "First cardinality project",
                    "/projects/first-cardinality",
                    ProjectInitializationParameters::default(),
                )
                .await?;
            transaction
                .create_project(
                    "Second cardinality project",
                    "/projects/second-cardinality",
                    ProjectInitializationParameters::default(),
                )
                .await
        })
        .await
        .unwrap();
    let mut optional_cardinality_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let optional_cardinality = optional_cardinality_transaction
        .sql_builder()
        .select(["id"])
        .from("projects")
        .fetch_optional()
        .await;
    assert!(optional_cardinality.is_err());
    assert!(optional_cardinality_transaction.commit().await.is_err());

    let mut multiple_fetch_one_transaction = test_infrastructure
        .infrastructure
        .make_transaction()
        .await
        .unwrap();
    let multiple_fetch_one = multiple_fetch_one_transaction
        .sql_builder()
        .select(["id"])
        .from("projects")
        .fetch_one()
        .await;
    assert!(multiple_fetch_one.is_err());
    assert!(multiple_fetch_one_transaction.commit().await.is_err());
}
