use super::*;

pub(crate) async fn insert_attachments(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
    attachment_inputs: Vec<AttachmentInput>,
    created_at: OffsetDateTime,
) -> InfrastructureResult<Vec<Attachment>> {
    let created_at_text = format_timestamp(created_at)?;
    let mut attachments = Vec::with_capacity(attachment_inputs.len());

    for (position, attachment_input) in attachment_inputs.into_iter().enumerate() {
        let attachment_id = AttachmentId::new();
        let metadata_json = serialize_json(&attachment_input.metadata)?;
        let byte_length = i64::try_from(attachment_input.byte_length).map_err(|source| {
            InfrastructureError::new(
                InfrastructureErrorKind::EntityValidationFailed,
                format!("attachment byte length exceeds SQLite range: {source}"),
            )
        })?;
        sqlx::query(
            "INSERT INTO attachments (id, message_id, position, file_name, media_type, byte_length, content_sha256, storage_reference, metadata_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(attachment_id.as_bytes().to_vec())
        .bind(message_id.as_bytes().to_vec())
        .bind(position as i64)
        .bind(&attachment_input.file_name)
        .bind(&attachment_input.media_type)
        .bind(byte_length)
        .bind(&attachment_input.content_sha256)
        .bind(&attachment_input.storage_reference)
        .bind(metadata_json)
        .bind(&created_at_text)
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| database_operation_error("failed to insert attachment", source))?;
        attachments.push(Attachment {
            id: attachment_id,
            message_id,
            position: position as u32,
            file_name: attachment_input.file_name,
            media_type: attachment_input.media_type,
            byte_length: attachment_input.byte_length,
            content_sha256: attachment_input.content_sha256,
            storage_reference: attachment_input.storage_reference,
            metadata: attachment_input.metadata,
            created_at,
        });
    }

    Ok(attachments)
}
