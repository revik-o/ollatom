use super::content::NativeMessage;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use filesystem::{FilesystemError, WorkspaceDirectory};
use llm::LlmError;
use std::path::{Path, PathBuf};

const MAXIMUM_ATTACHMENT_SIZE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_AGGREGATE_ATTACHMENT_SIZE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) struct AttachmentBudget {
    maximum_attachment_size_bytes: u64,
    maximum_aggregate_attachment_size_bytes: u64,
    aggregate_attachment_size_bytes: u64,
}

impl AttachmentBudget {
    pub(super) fn new(maximum_attachment_size_bytes: u64, maximum_aggregate_attachment_size_bytes: u64) -> Self {
        Self {
            maximum_attachment_size_bytes,
            maximum_aggregate_attachment_size_bytes,
            aggregate_attachment_size_bytes: 0,
        }
    }

    fn maximum_next_attachment_size_bytes(&self) -> u64 {
        self.maximum_attachment_size_bytes.min(
            self.maximum_aggregate_attachment_size_bytes
                .saturating_sub(self.aggregate_attachment_size_bytes),
        )
    }

    fn aggregate_limit_would_be_exceeded(&self) -> bool {
        self.maximum_aggregate_attachment_size_bytes
            .saturating_sub(self.aggregate_attachment_size_bytes)
            < self.maximum_attachment_size_bytes
    }

    fn charge(&mut self, attachment_size_bytes: u64) -> Result<(), LlmError> {
        if attachment_size_bytes > self.maximum_attachment_size_bytes {
            return Err(LlmError::InvalidRequest("attachment exceeds size limit".into()));
        }

        let aggregate_attachment_size_bytes = self
            .aggregate_attachment_size_bytes
            .checked_add(attachment_size_bytes)
            .ok_or_else(|| LlmError::InvalidRequest("attachment budget overflow".into()))?;

        if aggregate_attachment_size_bytes > self.maximum_aggregate_attachment_size_bytes {
            return Err(LlmError::InvalidRequest(
                "aggregate attachments exceed size limit".into(),
            ));
        }

        self.aggregate_attachment_size_bytes = aggregate_attachment_size_bytes;
        Ok(())
    }
}

impl Default for AttachmentBudget {
    fn default() -> Self {
        Self::new(MAXIMUM_ATTACHMENT_SIZE_BYTES, MAXIMUM_AGGREGATE_ATTACHMENT_SIZE_BYTES)
    }
}

enum AttachmentKind {
    Image,
    Audio,
}

pub(super) fn add_media(
    native_message: &mut NativeMessage,
    attachment_budget: &mut AttachmentBudget,
    media_type: &str,
    attachment_data: &[u8],
) -> Result<(), LlmError> {
    let attachment_size = u64::try_from(attachment_data.len())
        .map_err(|_| LlmError::InvalidRequest("attachment size is not representable".into()))?;
    let normalized_media_type = normalize_media_type(media_type);

    attachment_kind(normalized_media_type)?;
    attachment_budget.charge(attachment_size)?;

    let encoded_attachment = BASE64_STANDARD.encode(attachment_data);

    native_message.media.push(encoded_attachment);

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
        Err(LlmError::UnsupportedOption(format!("media type {media_type}")))
    }
}

pub(super) async fn read_attachment(
    path: &str,
    media_type: &str,
    attachment_budget: &AttachmentBudget,
) -> Result<Vec<u8>, LlmError> {
    attachment_kind(media_type)?;
    let path_ref = Path::new(path);
    let parent = path_ref.parent().unwrap_or_else(|| Path::new("."));

    let file_name = path_ref
        .file_name()
        .ok_or_else(|| LlmError::InvalidRequest("attachment path must name a regular file".into()))?;

    let workspace = WorkspaceDirectory::open(parent)
        .await
        .map_err(|error| LlmError::InvalidRequest(format!("cannot open attachment directory: {error}")))?;

    let attachment = workspace
        .file(PathBuf::from(file_name))
        .map_err(|error| LlmError::InvalidRequest(format!("invalid attachment path: {error}")))?;

    let maximum_read_size_bytes = attachment_budget.maximum_next_attachment_size_bytes();

    attachment
        .read_regular_file_bytes_bounded(maximum_read_size_bytes)
        .await
        .map_err(|error| match error {
            FilesystemError::FileSizeLimitExceeded { .. } if attachment_budget.aggregate_limit_would_be_exceeded() => {
                LlmError::InvalidRequest("aggregate attachments exceed size limit".into())
            }
            FilesystemError::FileSizeLimitExceeded { .. } => {
                LlmError::InvalidRequest("attachment exceeds size limit".into())
            }
            error => LlmError::InvalidRequest(format!("cannot read attachment: {error}")),
        })
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
