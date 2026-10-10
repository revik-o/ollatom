use llm::{
    ContentBlock, ConversationMessage, ConversationRole, LlmError, RunEvent, ToolCall, ToolEvent,
    ToolOutput, append_tool_round_to_history,
};

#[derive(Default)]
pub struct ConversationHistoryRecorder {
    completed_rounds: Vec<ConversationMessage>,
    current_round: ProviderRoundHistory,
}

#[derive(Default)]
struct ProviderRoundHistory {
    started: bool,
    response_text: String,
    reasoning_summary: String,
    tool_calls: Vec<ToolCall>,
    tool_outputs: Vec<ToolOutput>,
}

impl ConversationHistoryRecorder {
    pub fn record(&mut self, event: &RunEvent) -> Result<(), LlmError> {
        match event {
            RunEvent::ProviderRoundStarted { .. } => self.start_next_provider_round()?,
            RunEvent::ResponseDelta(response_delta) => {
                self.current_round.started = true;
                self.current_round.response_text.push_str(response_delta);
            }
            RunEvent::ReasoningSummaryDelta(reasoning_delta) => {
                self.current_round.started = true;
                self.current_round
                    .reasoning_summary
                    .push_str(reasoning_delta);
            }
            RunEvent::Tool(ToolEvent::Planned { call }) => {
                self.current_round.started = true;
                self.current_round.tool_calls.push(call.clone());
            }
            RunEvent::Tool(ToolEvent::Finished { output }) => {
                self.current_round.started = true;
                self.current_round.tool_outputs.push(output.clone());
            }
            _ => {}
        }

        Ok(())
    }

    pub fn into_conversation_history(
        mut self,
        stage_prompt: &str,
    ) -> Result<Vec<ConversationMessage>, LlmError> {
        self.finish_current_provider_round()?;
        let mut conversation_history = Vec::with_capacity(self.completed_rounds.len());
        conversation_history.reserve(1);
        conversation_history.push(ConversationMessage {
            role: ConversationRole::User,
            content: vec![ContentBlock::text(stage_prompt)],
        });

        conversation_history.append(&mut self.completed_rounds);

        Ok(conversation_history)
    }

    fn start_next_provider_round(&mut self) -> Result<(), LlmError> {
        self.finish_current_provider_round()?;
        self.current_round.started = true;

        Ok(())
    }

    fn finish_current_provider_round(&mut self) -> Result<(), LlmError> {
        if !self.current_round.started {
            return Ok(());
        }

        let completed_round = std::mem::take(&mut self.current_round);

        append_tool_round_to_history(
            &mut self.completed_rounds,
            completed_round.response_text,
            Some(completed_round.reasoning_summary),
            completed_round.tool_calls,
            completed_round.tool_outputs,
        )
    }
}
