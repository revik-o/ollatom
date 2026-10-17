use super::*;

#[tokio::test]
async fn invalid_action_transition_marks_manual_transaction_failed() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let mut transaction = test_infrastructure.infrastructure.make_transaction().await.unwrap();
    let user_message = transaction
        .add_message_from_user("Question", Vec::new(), &chat)
        .await
        .unwrap();
    let llm_message = transaction
        .begin_llm_message_by_chat_id_and_user_message_id(chat.id, user_message.id)
        .await
        .unwrap();
    let action = transaction
        .add_llm_action_to_message_by_id(
            llm_message.id,
            None,
            command_action_details(),
            status_event(LlmActionStatus::Pending),
        )
        .await
        .unwrap();

    let transition_error = transaction
        .append_llm_action_status_event(action.id, status_event(LlmActionStatus::Succeeded))
        .await
        .unwrap_err();
    assert_eq!(
        transition_error.kind(),
        InfrastructureErrorKind::InvalidLlmActionStatusTransition
    );
    assert_eq!(
        transaction.commit().await.unwrap_err().kind(),
        InfrastructureErrorKind::TransactionWasMarkedAsFailed
    );
}

#[tokio::test]
async fn typed_llm_reply_rejects_a_user_message_from_another_chat() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (project, first_chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let result = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            let second_chat = transaction
                .create_chat(
                    "Second chat",
                    project.id,
                    infrastructure::ChatInitializationParameters::default(),
                )
                .await?;
            let user_message = transaction
                .add_message_from_user("Question", Vec::new(), &first_chat)
                .await?;
            transaction
                .add_message_from_llm("Wrong chat", Vec::new(), user_message.id, second_chat.id)
                .await
        })
        .await;
    assert!(result.is_err());
}
