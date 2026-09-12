use super::content::NativeMessage;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use llm::LlmError;
use std::path::Path;

const MAXIMUM_ATTACHMENT_SIZE_BYTES: u64 = 32 * 1024 * 1024;

enum AttachmentKind {
    Image,
    Audio,
}

pub(super) fn add_media(
    native_message: &mut NativeMessage,
    media_type: &str,
    attachment_data: &[u8],
) -> Result<(), LlmError> {
    if u64::try_from(attachment_data.len()).unwrap_or(u64::MAX) > MAXIMUM_ATTACHMENT_SIZE_BYTES {
        return Err(LlmError::InvalidRequest(
            "attachment exceeds size limit".into(),
        ));
    }

    let normalized_media_type = normalize_media_type(media_type);
    let encoded_attachment = BASE64_STANDARD.encode(attachment_data);

    match attachment_kind(normalized_media_type)? {
        AttachmentKind::Image => native_message.images.push(encoded_attachment),
        AttachmentKind::Audio => native_message.audio.push(encoded_attachment),
    }

    Ok(())
}

fn normalize_media_type(media_type: &str) -> &str {
    media_type.split(';').next().unwrap_or(media_type).trim()
}

fn attachment_kind(media_type: &str) -> Result<AttachmentKind, LlmError> {
    if media_type.starts_with("image/") {
        Ok(AttachmentKind::Image)
    } else if media_type.starts_with("audio/") {
        Ok(AttachmentKind::Audio)
    } else {
        Err(LlmError::UnsupportedOption(format!(
            "media type {media_type}"
        )))
    }
}

pub(super) async fn read_attachment(path: &str, media_type: &str) -> Result<Vec<u8>, LlmError> {
    let path_ref = Path::new(path);
    let metadata = tokio::fs::symlink_metadata(path_ref)
        .await
        .map_err(|error| LlmError::InvalidRequest(format!("cannot inspect attachment: {error}")))?;

    if !metadata.file_type().is_file() {
        return Err(LlmError::InvalidRequest(
            "attachment is not a regular file".into(),
        ));
    }

    attachment_kind(media_type)?;

    if metadata.len() > MAXIMUM_ATTACHMENT_SIZE_BYTES {
        return Err(LlmError::InvalidRequest(
            "attachment exceeds size limit".into(),
        ));
    }

    tokio::fs::read(path_ref)
        .await
        .map_err(|error| LlmError::InvalidRequest(format!("cannot read attachment: {error}")))
}

pub(super) fn infer_media_type(path: &str, explicit: Option<&str>) -> Result<String, LlmError> {
    if let Some(media_type) = explicit {
        let normalized_media_type = normalize_media_type(media_type);
        attachment_kind(normalized_media_type)?;
        return Ok(normalized_media_type.to_string());
    }

    let inferred_media_type = mime_guess::from_path(path)
        .first_raw()
        .ok_or_else(|| LlmError::InvalidRequest("media type cannot be inferred".into()))?;

    attachment_kind(inferred_media_type)?;

    Ok(inferred_media_type.to_string())
}
