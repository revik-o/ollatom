use super::columns::{read_column, read_nonnegative_u64, read_u32};
use crate::database::{parse_timestamp, parse_uuid};
use crate::{Attachment, AttachmentId, InfrastructureError, InfrastructureErrorKind, InfrastructureResult, MessageId};
use sqlx::sqlite::SqliteRow;

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
        created_at: parse_timestamp(read_column::<&str>(database_row, "created_at")?)?,
    })
}
