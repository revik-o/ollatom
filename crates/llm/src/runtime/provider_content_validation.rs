use crate::{ContentBlock, LlmError, ProviderId};

pub(super) fn validate_context(
    selected_provider: &ProviderId,
    conversation_context: &[crate::ConversationMessage],
) -> Result<(), LlmError> {
    validate_provider_opaque_block_ownership(
        selected_provider,
        conversation_context
            .iter()
            .flat_map(|message| message.content.iter()),
    )
}

pub(super) fn validate_user_message(
    selected_provider: &ProviderId,
    user_message: &crate::UserMessage,
) -> Result<(), LlmError> {
    validate_provider_opaque_block_ownership(selected_provider, user_message.content.iter())
}

fn validate_provider_opaque_block_ownership<'a>(
    selected_provider: &ProviderId,
    content_blocks: impl IntoIterator<Item = &'a ContentBlock>,
) -> Result<(), LlmError> {
    for content_block in content_blocks {
        let ContentBlock::ProviderOpaque {
            provider: owning_provider,
            ..
        } = content_block
        else {
            continue;
        };

        if owning_provider != selected_provider {
            return Err(LlmError::InvalidRequest(format!(
                "opaque state for {owning_provider} cannot be sent to {selected_provider}"
            )));
        }
    }

    Ok(())
}
