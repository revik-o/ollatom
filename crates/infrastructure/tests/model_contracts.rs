use infrastructure::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::str::FromStr;
use time::OffsetDateTime;

#[test]
fn all_identifier_types_support_uuid_and_serde_contracts() {
    macro_rules! assert_identifier_contract {
        ($identifier_type:ty) => {{
            let identifier = <$identifier_type>::new();
            let uuid_value: uuid::Uuid = identifier.into();
            let parsed_identifier = <$identifier_type>::from_str(&uuid_value.to_string()).unwrap();
            assert_eq!(parsed_identifier, identifier);
            assert_eq!(identifier.to_string(), uuid_value.to_string());
            let serialized_identifier = serde_json::to_string(&identifier).unwrap();
            assert_eq!(
                serde_json::from_str::<$identifier_type>(&serialized_identifier).unwrap(),
                identifier
            );
        }};
    }

    assert_identifier_contract!(ProjectId);
    assert_identifier_contract!(ChatId);
    assert_identifier_contract!(MessageId);
    assert_identifier_contract!(AttachmentId);
    assert_identifier_contract!(LlmActionId);
    assert_identifier_contract!(LlmActionStatusEventId);
}

#[test]
fn defaults_builders_roles_and_action_models_round_trip() {
    assert_eq!(
        ProjectInitializationParameters::default().cpu_usage_percentage,
        100
    );
    assert_eq!(
        ChatInitializationParameters::default().gpu_usage_percentage,
        100
    );

    let project_id = ProjectId::new();
    let project_update = ProjectUpdateOptions::new(project_id)
        .with_name("updated")
        .with_path("/updated")
        .with_llm_thinking_enabled(true)
        .with_llm_context_optimization_enabled(true)
        .with_cpu_usage_percentage(75)
        .with_gpu_usage_percentage(65);
    let chat_update = ChatUpdateOptions::new(ChatId::new())
        .with_name("updated chat")
        .with_llm_thinking_enabled(true)
        .with_llm_context_optimization_enabled(true)
        .with_cpu_usage_percentage(55)
        .with_gpu_usage_percentage(45);
    let timestamp = OffsetDateTime::now_utc();
    let user_message = Message {
        id: MessageId::new(),
        chat_id: ChatId::new(),
        sequence_number: 1,
        contents: "question".to_owned(),
        attachments: vec![Attachment {
            id: AttachmentId::new(),
            message_id: MessageId::new(),
            position: 0,
            file_name: "file.txt".to_owned(),
            media_type: Some("text/plain".to_owned()),
            byte_length: 3,
            content_sha256: None,
            storage_reference: "file.txt".to_owned(),
            metadata: json!({"key": "value"}),
            created_at: timestamp,
        }],
        role_metadata: MessageRoleMetadata::User(UserMessageMetadata {
            user_revision_group_id: uuid::Uuid::new_v4(),
            user_revision_number: 1,
        }),
        validity: MessageValidity::Active,
        created_at: timestamp,
        updated_at: None,
        deprecated_at: None,
    };
    assert_eq!(user_message.role(), MessageRole::User);
    let llm_message = Message {
        role_metadata: MessageRoleMetadata::Llm(LlmMessageMetadata {
            llm_reply_to_user_message_id: user_message.id,
            llm_response_round_number: 1,
            llm_message_state: LlmMessageState::Completed,
        }),
        ..user_message.clone()
    };
    assert_eq!(llm_message.role(), MessageRole::Llm);

    let action = LlmAction {
        id: LlmActionId::new(),
        message_id: llm_message.id,
        sequence_number: 1,
        summary: Some("command".to_owned()),
        details: LlmActionDetails::Command(CommandActionDetails {
            command_text: "echo ok".to_owned(),
            working_directory: "/tmp".to_owned(),
            environment: BTreeMap::new(),
        }),
        status_events: vec![LlmActionStatusEvent {
            id: LlmActionStatusEventId::new(),
            llm_action_id: LlmActionId::new(),
            sequence_number: 1,
            status: LlmActionStatus::Succeeded,
            payload: Some(json!({"exit_code": 0})),
            occurred_at: timestamp,
        }],
        created_at: timestamp,
    };

    let _ = (project_update, chat_update);
    for value in [
        serde_json::to_value(ProjectInitializationParameters::default()).unwrap(),
        serde_json::to_value(ChatInitializationParameters::default()).unwrap(),
        serde_json::to_value(user_message).unwrap(),
        serde_json::to_value(llm_message).unwrap(),
        serde_json::to_value(action).unwrap(),
    ] {
        assert!(!value.is_null());
    }
}
