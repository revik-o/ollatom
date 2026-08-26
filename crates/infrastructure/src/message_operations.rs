use crate::database::{database_operation_error, format_timestamp, validate_nonblank_value};
use crate::mapping::{map_attachment_row, map_message_row, serialize_json};
use crate::{
    Attachment, AttachmentId, AttachmentInput, ChatId, CommandActionDetails, FileChangeDetails,
    FileChangeOperation, InfrastructureError, InfrastructureErrorKind, InfrastructureResult,
    LlmAction, LlmActionDetails, LlmActionId, LlmActionInput, LlmActionStatus,
    LlmActionStatusEvent, LlmActionStatusEventId, LlmActionStatusEventInput, LlmMessageState,
    Message, MessageId, MessageRoleMetadata, MessageValidity, ToolCallActionDetails,
    UserMessageMetadata,
};
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};
use time::OffsetDateTime;

const MESSAGE_COLUMNS: &str = "id, chat_id, sequence_number, role, contents, user_revision_group_id, user_revision_number, llm_reply_to_user_message_id, llm_response_round_number, llm_message_state, validity, created_at, updated_at, deprecated_at";

mod action_details;
mod action_journal;
mod attachments;
mod llm_messages;
mod loading;
mod sequence_numbers;
mod user_messages;
mod validation;

pub(crate) use action_details::*;
pub(crate) use action_journal::*;
pub(crate) use attachments::*;
pub(crate) use llm_messages::*;
pub(crate) use loading::*;
pub(crate) use sequence_numbers::*;
pub(crate) use user_messages::*;
pub(crate) use validation::*;
