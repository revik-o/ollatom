use crate::database::{
    database_operation_error, parse_optional_timestamp, parse_timestamp, parse_uuid,
};
use crate::{
    Attachment, AttachmentId, Chat, ChatId, InfrastructureError, InfrastructureErrorKind,
    InfrastructureResult, LlmMessageMetadata, LlmMessageState, Message, MessageId,
    MessageRoleMetadata, MessageValidity, Project, ProjectId, UserMessageMetadata,
};
use serde_json::Value;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

pub(crate) fn map_project_row(database_row: &SqliteRow) -> InfrastructureResult<Project> {
    Ok(Project {
        id: ProjectId::from(parse_uuid(read_column(database_row, "id")?)?),
        name: read_column(database_row, "name")?,
        path: read_column(database_row, "path")?,
        llm_thinking_enabled: read_integer_boolean(database_row, "llm_thinking_enabled")?,
        llm_context_optimization_enabled: read_integer_boolean(
            database_row,
            "llm_context_optimization_enabled",
        )?,
        cpu_usage_percentage: read_u8(database_row, "cpu_usage_percentage")?,
        gpu_usage_percentage: read_u8(database_row, "gpu_usage_percentage")?,
        created_at: parse_timestamp(&read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_timestamp(&read_column::<&str>(database_row, "updated_at")?)?,
    })
}

pub(crate) fn map_chat_row(database_row: &SqliteRow) -> InfrastructureResult<Chat> {
    Ok(Chat {
        id: ChatId::from(parse_uuid(read_column(database_row, "id")?)?),
        project_id: ProjectId::from(parse_uuid(read_column(database_row, "project_id")?)?),
        name: read_column(database_row, "name")?,
        llm_thinking_enabled: read_integer_boolean(database_row, "llm_thinking_enabled")?,
        llm_context_optimization_enabled: read_integer_boolean(
            database_row,
            "llm_context_optimization_enabled",
        )?,
        cpu_usage_percentage: read_u8(database_row, "cpu_usage_percentage")?,
        gpu_usage_percentage: read_u8(database_row, "gpu_usage_percentage")?,
        created_at: parse_timestamp(&read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_timestamp(&read_column::<&str>(database_row, "updated_at")?)?,
    })
}

pub(crate) fn map_attachment_row(database_row: &SqliteRow) -> InfrastructureResult<Attachment> {
    let metadata_json: String = read_column(database_row, "metadata_json")?;
    let byte_length = read_nonnegative_u64(database_row, "byte_length")?;
    Ok(Attachment {
        id: AttachmentId::from(parse_uuid(read_column(database_row, "id")?)?),
        message_id: MessageId::from(parse_uuid(read_column(database_row, "message_id")?)?),
        position: read_u32(database_row, "position")?,
        file_name: read_column(database_row, "file_name")?,
        media_type: read_column(database_row, "media_type")?,
        byte_length,
        content_sha256: read_column(database_row, "content_sha256")?,
        storage_reference: read_column(database_row, "storage_reference")?,
        metadata: serde_json::from_str(&metadata_json).map_err(|source| {
            InfrastructureError::new(
                InfrastructureErrorKind::DatabaseOperationFailed,
                format!("failed to deserialize attachment metadata: {source}"),
            )
        })?,
        created_at: parse_timestamp(&read_column::<&str>(database_row, "created_at")?)?,
    })
}

pub(crate) fn map_message_row(
    database_row: &SqliteRow,
    attachments: Vec<Attachment>,
) -> InfrastructureResult<Message> {
    let role: String = read_column(database_row, "role")?;
    let role_metadata = match role.as_str() {
        "user" => MessageRoleMetadata::User(UserMessageMetadata {
            user_revision_group_id: parse_uuid(read_required_optional_column(
                database_row,
                "user_revision_group_id",
            )?)?,
            user_revision_number: read_required_optional_u32(database_row, "user_revision_number")?,
        }),
        "llm" => MessageRoleMetadata::Llm(LlmMessageMetadata {
            llm_reply_to_user_message_id: MessageId::from(parse_uuid(
                read_required_optional_column(database_row, "llm_reply_to_user_message_id")?,
            )?),
            llm_response_round_number: read_required_optional_u32(
                database_row,
                "llm_response_round_number",
            )?,
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
        sequence_number: read_nonnegative_u64(database_row, "sequence_number")?,
        contents: read_column(database_row, "contents")?,
        attachments,
        role_metadata,
        validity,
        created_at: parse_timestamp(&read_column::<&str>(database_row, "created_at")?)?,
        updated_at: parse_optional_timestamp(read_column(database_row, "updated_at")?)?,
        deprecated_at: parse_optional_timestamp(read_column(database_row, "deprecated_at")?)?,
    })
}

fn parse_llm_message_state(stored_state: &str) -> InfrastructureResult<LlmMessageState> {
    match stored_state {
        "in_progress" => Ok(LlmMessageState::InProgress),
        "completed" => Ok(LlmMessageState::Completed),
        "failed" => Ok(LlmMessageState::Failed),
        "cancelled" => Ok(LlmMessageState::Cancelled),
        unexpected_state => Err(InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored LLM message state '{unexpected_state}' is invalid"),
        )),
    }
}

fn read_column<'database_row, ColumnValue>(
    database_row: &'database_row SqliteRow,
    column_name: &str,
) -> InfrastructureResult<ColumnValue>
where
    ColumnValue: sqlx::Decode<'database_row, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    database_row
        .try_get(column_name)
        .map_err(|source| database_operation_error("failed to decode database row", source))
}

fn read_required_optional_column<'database_row, ColumnValue>(
    database_row: &'database_row SqliteRow,
    column_name: &str,
) -> InfrastructureResult<ColumnValue>
where
    ColumnValue: sqlx::Decode<'database_row, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    read_column::<Option<ColumnValue>>(database_row, column_name)?.ok_or_else(|| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("required role-specific column '{column_name}' is null"),
        )
    })
}

fn read_integer_boolean(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<bool> {
    Ok(read_column::<i64>(database_row, column_name)? != 0)
}

fn read_u8(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<u8> {
    let stored_value: i64 = read_column(database_row, column_name)?;
    u8::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored column '{column_name}' is outside the u8 range: {source}"),
        )
    })
}

fn read_u32(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<u32> {
    let stored_value: i64 = read_column(database_row, column_name)?;
    u32::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored column '{column_name}' is outside the u32 range: {source}"),
        )
    })
}

fn read_required_optional_u32(
    database_row: &SqliteRow,
    column_name: &str,
) -> InfrastructureResult<u32> {
    let stored_value: i64 = read_required_optional_column(database_row, column_name)?;
    u32::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored column '{column_name}' is outside the u32 range: {source}"),
        )
    })
}

fn read_nonnegative_u64(database_row: &SqliteRow, column_name: &str) -> InfrastructureResult<u64> {
    let stored_value: i64 = read_column(database_row, column_name)?;
    u64::try_from(stored_value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("stored column '{column_name}' is negative: {source}"),
        )
    })
}

pub(crate) fn serialize_json(value: &Value) -> InfrastructureResult<String> {
    serde_json::to_string(value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to serialize JSON value: {source}"),
        )
    })
}
