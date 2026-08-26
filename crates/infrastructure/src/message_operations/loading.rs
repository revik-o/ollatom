use super::*;

pub(crate) async fn load_message_by_id(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
) -> InfrastructureResult<Option<Message>> {
    let query_text = format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = ?");
    let database_row = sqlx::query(AssertSqlSafe(query_text))
        .bind(message_id.as_bytes().to_vec())
        .fetch_optional(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to get message by ID", source))?;
    let Some(database_row) = database_row else {
        return Ok(None);
    };
    let attachments = load_attachments(database_transaction, message_id).await?;
    map_message_row(&database_row, attachments).map(Some)
}

pub(crate) async fn load_attachments(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
) -> InfrastructureResult<Vec<Attachment>> {
    let database_rows = sqlx::query(
        "SELECT id, message_id, position, file_name, media_type, byte_length, content_sha256, storage_reference, metadata_json, created_at FROM attachments WHERE message_id = ? ORDER BY position",
    )
    .bind(message_id.as_bytes().to_vec())
    .fetch_all(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to load message attachments", source))?;
    database_rows.iter().map(map_attachment_row).collect()
}
