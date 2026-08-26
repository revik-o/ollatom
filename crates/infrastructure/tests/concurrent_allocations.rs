mod support;

use infrastructure::{LlmActionInput, MessageRoleMetadata};
use std::sync::Arc;
use std::time::Duration;
use support::{create_initialized_test_infrastructure, create_project_with_chat};
use tokio::sync::Barrier;
use tokio::task::JoinSet;

#[tokio::test]
async fn concurrent_user_messages_receive_unique_active_sequences() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let writer_count = 8;
    let start_barrier = Arc::new(Barrier::new(writer_count));
    let mut writers = JoinSet::new();

    for writer_number in 0..writer_count {
        let infrastructure = test_infrastructure.infrastructure.clone();
        let chat = chat.clone();
        let start_barrier = start_barrier.clone();
        writers.spawn(async move {
            start_barrier.wait().await;
            infrastructure
                .execute_db_actions(async |transaction| {
                    transaction
                        .add_message_from_user(
                            format!("Concurrent question {writer_number}"),
                            Vec::new(),
                            &chat,
                        )
                        .await
                })
                .await
                .map(|message| message.sequence_number)
        });
    }

    let mut sequence_numbers = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(result) = writers.join_next().await {
            sequence_numbers.push(result.unwrap().unwrap());
        }
    })
    .await
    .expect("concurrent sequence allocation timed out");

    sequence_numbers.sort_unstable();
    assert_eq!(
        sequence_numbers,
        (1..=writer_count as u64).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn concurrent_llm_messages_receive_unique_response_rounds() {
    let test_infrastructure = create_initialized_test_infrastructure().await;
    let (_, chat) = create_project_with_chat(&test_infrastructure.infrastructure).await;
    let user_message = test_infrastructure
        .infrastructure
        .execute_db_actions(async |transaction| {
            transaction
                .add_message_from_user("Concurrent request", Vec::new(), &chat)
                .await
        })
        .await
        .unwrap();
    let writer_count = 8;
    let start_barrier = Arc::new(Barrier::new(writer_count));
    let mut writers = JoinSet::new();

    for writer_number in 0..writer_count {
        let infrastructure = test_infrastructure.infrastructure.clone();
        let chat = chat.clone();
        let user_message = user_message.clone();
        let start_barrier = start_barrier.clone();
        writers.spawn(async move {
            start_barrier.wait().await;
            infrastructure
                .execute_db_actions(async |transaction| {
                    transaction
                        .add_message_from_llm(
                            format!("Concurrent answer {writer_number}"),
                            Vec::<LlmActionInput>::new(),
                            &user_message,
                            &chat,
                        )
                        .await
                })
                .await
                .map(|message| match message.role_metadata {
                    MessageRoleMetadata::Llm(metadata) => metadata.llm_response_round_number,
                    MessageRoleMetadata::User(_) => 0,
                })
        });
    }

    let mut response_round_numbers = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(result) = writers.join_next().await {
            response_round_numbers.push(result.unwrap().unwrap());
        }
    })
    .await
    .expect("concurrent response-round allocation timed out");

    response_round_numbers.sort_unstable();
    assert_eq!(
        response_round_numbers,
        (1..=writer_count as u32).collect::<Vec<_>>()
    );
}
