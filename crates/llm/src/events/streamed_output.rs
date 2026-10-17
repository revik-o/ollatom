use crate::{AssistantRound, FinishReason, ToolCall, Usage};

#[derive(Clone, Default)]
pub(crate) struct StreamedOutput {
    pub assistant_rounds: Vec<AssistantRound>,
    pub usage: Usage,
    pub(super) current_round: Option<AssistantRound>,
}

impl StreamedOutput {
    pub(crate) fn assistant_round_snapshot(&self) -> Vec<AssistantRound> {
        let mut assistant_rounds = self.assistant_rounds.clone();

        if let Some(current_round) = &self.current_round {
            assistant_rounds.push(current_round.clone());
        }

        assistant_rounds
    }

    pub(super) fn finish_current_round(&mut self) {
        if let Some(current_round) = self.current_round.take() {
            self.assistant_rounds.push(current_round);
        }
    }

    pub(super) fn add_tool_call(&mut self, call: &ToolCall) {
        let target_round = self.current_round.as_mut().or_else(|| self.assistant_rounds.last_mut());

        if let Some(target_round) = target_round
            && !target_round
                .tool_calls
                .iter()
                .any(|existing_call| existing_call.id == call.id)
        {
            target_round.tool_calls.push(call.clone());
        }
    }

    pub(super) fn record_round_completion(
        &mut self,
        round_number: u16,
        finish_reason: FinishReason,
        tool_calls: &[ToolCall],
        usage: &Usage,
    ) {
        let target_round = self
            .current_round
            .as_mut()
            .filter(|current_round| current_round.round_number == round_number)
            .or_else(|| {
                self.assistant_rounds
                    .iter_mut()
                    .rev()
                    .find(|completed_round| completed_round.round_number == round_number)
            });

        if let Some(target_round) = target_round {
            target_round.finish_reason = finish_reason;
            target_round.usage = usage.clone();

            for tool_call in tool_calls {
                if !target_round
                    .tool_calls
                    .iter()
                    .any(|existing_call| existing_call.id == tool_call.id)
                {
                    target_round.tool_calls.push(tool_call.clone());
                }
            }
        }
    }
}
