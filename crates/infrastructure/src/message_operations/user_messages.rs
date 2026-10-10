use super::*;

pub(crate) async fn add_message_from_user(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_contents: String,
    attachment_inputs: Vec<AttachmentInput>,
    chat_id: ChatId,
) -> InfrastructureResult<Message> {
    validate_chat_exists(database_transaction, chat_id).await?;
    validate_message_contents_and_attachments(&message_contents, &attachment_inputs)?;
    validate_attachment_inputs(&attachment_inputs)?;
    validate_attachment_integrity_metadata(&attachment_inputs)?;

    let message_id = MessageId::new();
    let sequence_number =
        allocate_next_active_message_sequence_number(database_transaction, chat_id).await?;
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;

    sqlx::query(
        "INSERT INTO messages (id, chat_id, sequence_number, role, contents, user_revision_group_id, user_revision_number, llm_reply_to_user_message_id, llm_response_round_number, llm_message_state, validity, created_at, updated_at, deprecated_at) VALUES (?, ?, ?, 'user', ?, ?, 1, NULL, NULL, NULL, 'active', ?, NULL, NULL)",
    )
    .bind(message_id.as_bytes().to_vec())
    .bind(chat_id.as_bytes().to_vec())
    .bind(i64::from(sequence_number))
    .bind(&message_contents)
    .bind(message_id.as_bytes().to_vec())
    .bind(&created_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to add user message", source))?;

    let attachments = insert_attachments(
        database_transaction,
        message_id,
        attachment_inputs,
        created_at,
    )
    .await?;

    Ok(Message {
        id: message_id,
        chat_id,
        sequence_number,
        contents: message_contents,
        attachments,
        role_metadata: MessageRoleMetadata::User(UserMessageMetadata {
            user_revision_group_id: message_id.as_uuid(),
            user_revision_number: 1,
        }),
        validity: MessageValidity::Active,
        created_at,
        updated_at: None,
        deprecated_at: None,
    })
}

fn validate_attachment_integrity_metadata(
    attachment_inputs: &[AttachmentInput],
) -> InfrastructureResult<()> {
    for attachment_input in attachment_inputs {
        if let Some(media_type) = attachment_input.media_type.as_deref() {
            validate_nonblank_value("attachment media type", media_type)?;
        }

        if let Some(content_sha256) = attachment_input.content_sha256.as_deref()
            && !is_lowercase_sha256_hexadecimal(content_sha256)
        {
            return Err(InfrastructureError::new(
                InfrastructureErrorKind::EntityValidationFailed,
                "attachment SHA-256 must contain 64 lowercase hexadecimal characters",
            ));
        }
    }

    Ok(())
}

fn is_lowercase_sha256_hexadecimal(content_sha256: &str) -> bool {
    content_sha256.len() == 64
        && content_sha256.bytes().all(|content_sha256_byte| {
            content_sha256_byte.is_ascii_digit() || (b'a'..=b'f').contains(&content_sha256_byte)
        })
}

pub(crate) async fn edit_message_from_user(
    database_transaction: &mut Transaction<'static, Sqlite>,
    new_message_contents: String,
    message_id: MessageId,
) -> InfrastructureResult<Option<Message>> {
    let Some(old_message) = load_message_by_id(database_transaction, message_id).await? else {
        return Ok(None);
    };

    let MessageRoleMetadata::User(user_message_metadata) = &old_message.role_metadata else {
        return Err(invalid_message_operation(
            "only a user message can be edited",
        ));
    };

    if old_message.validity != MessageValidity::Active {
        return Err(invalid_message_operation(
            "only an active user message can be edited",
        ));
    }

    if new_message_contents.trim().is_empty() && old_message.attachments.is_empty() {
        return Err(InfrastructureError::new(
            InfrastructureErrorKind::EntityValidationFailed,
            "edited user message must contain text or at least one attachment",
        ));
    }

    let updated_at = OffsetDateTime::now_utc();
    let updated_at_text = format_timestamp(updated_at)?;

    cancel_unfinished_actions_in_active_suffix(
        database_transaction,
        old_message.chat_id,
        old_message.sequence_number,
    )
    .await?;

    sqlx::query(
        "UPDATE messages SET validity = 'deprecated', deprecated_at = ? WHERE chat_id = ? AND sequence_number >= ? AND validity = 'active'",
    )
    .bind(&updated_at_text)
    .bind(old_message.chat_id.as_bytes().to_vec())
    .bind(i64::from(old_message.sequence_number))
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| {
        database_operation_error("failed to deprecate active conversation suffix", source)
    })?;

    let revised_message_id = MessageId::new();
    let revised_user_revision_number = user_message_metadata
        .user_revision_number
        .checked_add(1)
        .ok_or_else(|| {
            InfrastructureError::new(
                InfrastructureErrorKind::InvalidMessageOperation,
                "user message revision exceeds the u32 range",
            )
        })?;

    let created_at_text = format_timestamp(old_message.created_at)?;
    sqlx::query(
        "INSERT INTO messages (id, chat_id, sequence_number, role, contents, user_revision_group_id, user_revision_number, llm_reply_to_user_message_id, llm_response_round_number, llm_message_state, validity, created_at, updated_at, deprecated_at) VALUES (?, ?, ?, 'user', ?, ?, ?, NULL, NULL, NULL, 'active', ?, ?, NULL)",
    )
    .bind(revised_message_id.as_bytes().to_vec())
    .bind(old_message.chat_id.as_bytes().to_vec())
    .bind(i64::from(old_message.sequence_number))
    .bind(&new_message_contents)
    .bind(user_message_metadata.user_revision_group_id.as_bytes().to_vec())
    .bind(i64::from(revised_user_revision_number))
    .bind(&created_at_text)
    .bind(&updated_at_text)
    .execute(&mut **database_transaction)
    .await
    .map_err(|source| database_operation_error("failed to insert edited user message", source))?;

    let attachment_inputs = old_message
        .attachments
        .iter()
        .map(|attachment| AttachmentInput {
            file_name: attachment.file_name.clone(),
            media_type: attachment.media_type.clone(),
            byte_length: attachment.byte_length,
            content_sha256: attachment.content_sha256.clone(),
            storage_reference: attachment.storage_reference.clone(),
            metadata: attachment.metadata.clone(),
        })
        .collect();

    let attachments = insert_attachments(
        database_transaction,
        revised_message_id,
        attachment_inputs,
        updated_at,
    )
    .await?;

    Ok(Some(Message {
        id: revised_message_id,
        chat_id: old_message.chat_id,
        sequence_number: old_message.sequence_number,
        contents: new_message_contents,
        attachments,
        role_metadata: MessageRoleMetadata::User(UserMessageMetadata {
            user_revision_group_id: user_message_metadata.user_revision_group_id,
            user_revision_number: revised_user_revision_number,
        }),
        validity: MessageValidity::Active,
        created_at: old_message.created_at,
        updated_at: Some(updated_at),
        deprecated_at: None,
    }))
}

pub(crate) async fn delete_message_from_user(
    database_transaction: &mut Transaction<'static, Sqlite>,
    message_id: MessageId,
) -> InfrastructureResult<Option<Message>> {
    let Some(message) = load_message_by_id(database_transaction, message_id).await? else {
        return Ok(None);
    };

    if !matches!(message.role_metadata, MessageRoleMetadata::User(_)) {
        return Err(invalid_message_operation(
            "only a user message can start conversation deletion",
        ));
    }

    if message.validity != MessageValidity::Active {
        return Err(invalid_message_operation(
            "only an active user message can start conversation deletion",
        ));
    }

    sqlx::query("DELETE FROM messages WHERE chat_id = ? AND sequence_number >= ?")
        .bind(message.chat_id.as_bytes().to_vec())
        .bind(i64::from(message.sequence_number))
        .execute(&mut **database_transaction)
        .await
        .map_err(|source| {
            database_operation_error("failed to delete conversation suffix", source)
        })?;

    Ok(Some(message))
}
