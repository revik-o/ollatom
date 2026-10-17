use super::columns::{
    parse_llm_message_state, read_column, read_required_optional_column, read_required_optional_u32, read_u32,
};
use crate::database::{parse_optional_timestamp, parse_timestamp, parse_uuid};
use crate::{
    Attachment, ChatId, InfrastructureError, InfrastructureErrorKind, InfrastructureResult, LlmMessageMetadata, Message,
    MessageId, MessageRoleMetadata, MessageValidity, UserMessageMetadata,
};
use sqlx::sqlite::SqliteRow;

pub(crate) fn map_message_row(database_row: &SqliteRow, attachments: Vec<Attachment>) -> InfrastructureResult<Message> {
    let role: String = read_column(database_row, "role")?;

    let role_metadata = match role.as_str() {
        "user" => MessageRoleMetadata::User(UserMessageMetadata {
            user_revision_group_id: parse_uuid(read_required_optional_column(database_row, "user_revision_group_id")?)?,
            user_revision_number: read_required_optional_u32(database_row, "user_revision_number")?,
        }),
        "llm" => MessageRoleMetadata::Llm(LlmMessageMetadata {
            llm_reply_to_user_message_id: MessageId::from(parse_uuid(read_required_optional_column(
                database_row,
                "llm_reply_to_user_message_id",
            )?)?),
            llm_response_round_number: read_required_optional_u32(database_row, "llm_response_round_number")?,
            llm_message_state: parse_llm_message_state(&read_required_optional_column::<String>(
                database_row,
                "llm_message_state",
            )?)?,
        }),
        unexpected_role => {
            return Err(InfrastructureError::new(
                InfrastructureErrorKind::DatabaseOperationFailed,
                format!("stored message role '{unexpected_role}' is invalid"),
            ));
        }
    };

    let validity = match read_column::<String>(database_row, "validity")?.as_str() {
        "active" => MessageValidity::Active,
        "deprecated" => MessageValidity::Deprecated,
        unexpected_validity => {
            return Err(InfrastructureError::new(
                InfrastructureErrorKind::DatabaseOperationFailed,
                format!("stored message validity '{unexpected_validity}' is invalid"),
            ));
        }
    };

    Ok(Message {
        id: MessageId::from(parse_uuid(read_column(database_row, "id")?)?),
        chat_id: ChatId::from(parse_uuid(read_column(database_row, "chat_id")?)?),
        sequence_number: read_u32(database_row, "sequence_number")?,
        contents: read_column(database_row, "contents")?,
        attachments,
        role_metadata,
        validity,
        created_at: parse_timestamp(read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_optional_timestamp(read_column(database_row, "updated_at")?)?,
        deprecated_at: parse_optional_timestamp(read_column(database_row, "deprecated_at")?)?,
    })
}
