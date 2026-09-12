use llm::{
    ContentBlock, ConversationMessage, ConversationRole, RunEvent, ToolCall, ToolEvent, ToolOutput,
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
    pub fn record(&mut self, event: &RunEvent) {
        match event {
            RunEvent::ProviderRoundStarted { .. } => self.start_next_provider_round(),
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
    }

    #[must_use]
    pub fn into_conversation_history(mut self, stage_prompt: &str) -> Vec<ConversationMessage> {
        self.finish_current_provider_round();
        let mut conversation_history = Vec::with_capacity(self.completed_rounds.len() + 1);
        conversation_history.push(ConversationMessage {
            role: ConversationRole::User,
            content: vec![ContentBlock::text(stage_prompt)],
        });
        conversation_history.append(&mut self.completed_rounds);

        conversation_history
    }

    fn start_next_provider_round(&mut self) {
        self.finish_current_provider_round();
        self.current_round.started = true;
    }

    fn finish_current_provider_round(&mut self) {
        if !self.current_round.started {
            return;
        }

        let completed_round = std::mem::take(&mut self.current_round);
        let mut assistant_content = Vec::with_capacity(completed_round.tool_calls.len() + 1);

        if !completed_round.response_text.is_empty() {
            assistant_content.push(ContentBlock::text(completed_round.response_text));
        }

        if !completed_round.reasoning_summary.is_empty() {
            assistant_content.push(ContentBlock::ReasoningSummary {
                text: completed_round.reasoning_summary,
            });
        }

        assistant_content.extend(
            completed_round
                .tool_calls
                .into_iter()
                .map(|call| ContentBlock::ToolCall { call }),
        );

        if !assistant_content.is_empty() {
            self.completed_rounds.push(ConversationMessage {
                role: ConversationRole::Assistant,
                content: assistant_content,
            });
        }

        for tool_output in completed_round.tool_outputs {
            self.completed_rounds.push(ConversationMessage {
                role: ConversationRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    output: tool_output,
                }],
            });
        }
    }
}
