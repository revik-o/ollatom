use crate::content_history::{native_messages_from_history, native_messages_from_history_with_budget};
use crate::content_media::AttachmentBudget;
use llm::{ContentBlock, ConversationMessage, ConversationRole, ToolCall, ToolOutput};

#[tokio::test]
async fn native_history_rejects_replayed_and_unmatched_tool_results() {
    let call = assistant_tool_call("call-1");
    let result = tool_result("call-1");

    let replayed_result_history = vec![call.clone(), result.clone(), result.clone()];
    assert!(
        native_messages_from_history(None, &replayed_result_history, false)
            .await
            .is_err()
    );

    let reused_identifier_history = vec![call.clone(), result, assistant_tool_call("call-1")];
    assert!(
        native_messages_from_history(None, &reused_identifier_history, false)
            .await
            .is_err()
    );

    let unmatched_call_history = vec![call];
    assert!(
        native_messages_from_history(None, &unmatched_call_history, false)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_history_enforces_aggregate_attachment_budget_across_messages() {
    let history = vec![
        user_message(ContentBlock::Binary {
            media_type: "image/png".into(),
            filename: None,
            data: vec![1; 4],
        }),
        user_message(ContentBlock::Binary {
            media_type: "image/png".into(),
            filename: None,
            data: vec![2; 2],
        }),
    ];
    let mut attachment_budget = AttachmentBudget::new(4, 5);

    let error = native_messages_from_history_with_budget(None, &history, false, &mut attachment_budget)
        .await
        .unwrap_err();

    assert!(matches!(error, llm::LlmError::InvalidRequest(message) if message.contains("aggregate attachments")));
}

#[tokio::test]
async fn file_attachments_are_read_with_the_remaining_request_budget() {
    let attachment_directory = tempfile::tempdir().unwrap();
    let attachment_path = attachment_directory.path().join("image.png");
    std::fs::write(&attachment_path, [0; 6]).unwrap();
    let history = vec![
        user_message(ContentBlock::Binary {
            media_type: "image/png".into(),
            filename: None,
            data: vec![1; 4],
        }),
        user_message(ContentBlock::File {
            path: attachment_path.to_string_lossy().into_owned(),
            media_type: Some("image/png".into()),
        }),
    ];
    let mut attachment_budget = AttachmentBudget::new(4, 5);

    let error = native_messages_from_history_with_budget(None, &history, false, &mut attachment_budget)
        .await
        .unwrap_err();

    assert!(matches!(error, llm::LlmError::InvalidRequest(message) if message.contains("aggregate attachments")));
}

fn user_message(block: ContentBlock) -> ConversationMessage {
    ConversationMessage {
        role: ConversationRole::User,
        content: vec![block],
    }
}

fn assistant_tool_call(call_identifier: &str) -> ConversationMessage {
    ConversationMessage {
        role: ConversationRole::Assistant,
        content: vec![ContentBlock::ToolCall {
            call: ToolCall {
                id: call_identifier.into(),
                name: "echo".into(),
                arguments: serde_json::json!({"value": call_identifier}),
            },
        }],
    }
}

fn tool_result(call_identifier: &str) -> ConversationMessage {
    ConversationMessage {
        role: ConversationRole::Tool,
        content: vec![ContentBlock::ToolResult {
            output: ToolOutput::success(call_identifier, serde_json::json!("done")),
        }],
    }
}
