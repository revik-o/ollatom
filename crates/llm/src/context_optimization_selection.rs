use crate::{ContentBlock, ConversationMessage};

const RECENT_MODEL_VISIBLE_MESSAGE_COUNT: usize = 6;
const RECENT_TOOL_ROUND_COUNT: usize = 2;

pub(crate) fn recent_history_start_index(
    history: &[ConversationMessage],
    minimum_start_index: usize,
) -> usize {
    let message_count_start_index = history
        .len()
        .saturating_sub(RECENT_MODEL_VISIBLE_MESSAGE_COUNT)
        .max(minimum_start_index);
    let tool_round_start_index =
        oldest_retained_tool_round_start_index(history, minimum_start_index)
            .unwrap_or(message_count_start_index);

    message_count_start_index
        .min(tool_round_start_index)
        .max(minimum_start_index)
}

fn oldest_retained_tool_round_start_index(
    history: &[ConversationMessage],
    minimum_start_index: usize,
) -> Option<usize> {
    let mut oldest_retained_start_index = None;
    let mut retained_tool_round_count = 0;

    for message_index in (minimum_start_index..history.len()).rev() {
        if !message_contains_tool_call(&history[message_index]) {
            continue;
        }

        oldest_retained_start_index = Some(message_index);
        retained_tool_round_count += 1;

        if retained_tool_round_count == RECENT_TOOL_ROUND_COUNT {
            break;
        }
    }

    oldest_retained_start_index
}

fn message_contains_tool_call(message: &ConversationMessage) -> bool {
    message
        .content
        .iter()
        .any(|content| matches!(content, ContentBlock::ToolCall { .. }))
}
