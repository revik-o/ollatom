use crate::{BoxFuture, RequiredCapability, RunId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InteractionId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    Text,
    SingleChoice,
    MultipleChoice,
    Confirmation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub kind: QuestionKind,
    pub choices: Vec<String>,
    pub allow_free_form: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AskUserRequest {
    pub questions: Vec<Question>,
}

impl AskUserRequest {
    pub fn validate(&self) -> Result<(), crate::LlmError> {
        if self.questions.is_empty() {
            return invalid_interaction("ask_user requires at least one question");
        }

        let mut ids = BTreeSet::new();

        for question in &self.questions {
            if question.id.trim().is_empty() || question.prompt.trim().is_empty() {
                return invalid_interaction("question IDs and prompts cannot be empty");
            }

            if !ids.insert(question.id.as_str()) {
                return invalid_interaction("question IDs must be unique");
            }

            let requires_choices = matches!(
                question.kind,
                QuestionKind::SingleChoice | QuestionKind::MultipleChoice
            );

            if requires_choices && question.choices.is_empty() {
                return invalid_interaction("choice questions require suggested choices");
            }
        }

        Ok(())
    }

    pub fn validate_answers(&self, answers: &[QuestionAnswer]) -> Result<(), crate::LlmError> {
        let mut answered = BTreeSet::new();

        for answer in answers {
            let question = self
                .questions
                .iter()
                .find(|question| question.id == answer.question_id)
                .ok_or_else(|| {
                    invalid_interaction_error("answer references an unknown question")
                })?;

            if !answered.insert(answer.question_id.as_str()) {
                return invalid_interaction("each question may be answered only once");
            }

            if answer.free_form.is_some() && !question.allow_free_form {
                return invalid_interaction("free-form text is not allowed for this question");
            }

            let has_restricted_choices = !question.choices.is_empty();
            let contains_unknown_choice = has_restricted_choices
                && answer
                    .values
                    .iter()
                    .any(|value| !question.choices.contains(value));

            if contains_unknown_choice {
                return invalid_interaction(
                    "answer contains a value outside the suggested choices",
                );
            }

            let accepts_single_value = matches!(
                question.kind,
                QuestionKind::SingleChoice | QuestionKind::Confirmation
            );

            if accepts_single_value && answer.values.len() > 1 {
                return invalid_interaction("this question accepts only one value");
            }

            if answer.values.is_empty() && answer.free_form.is_none() {
                return invalid_interaction("an answer cannot be empty");
            }
        }

        if answered.len() != self.questions.len() {
            return invalid_interaction("every question requires an answer");
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub question_id: String,
    pub values: Vec<String>,
    pub free_form: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub run_id: RunId,
    pub tool_name: String,
    pub call_id: String,
    pub capabilities: Vec<RequiredCapability>,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "request", rename_all = "snake_case")]
pub enum InteractionRequest {
    Approval {
        id: InteractionId,
        request: ApprovalRequest,
    },
    AskUser {
        id: InteractionId,
        request: AskUserRequest,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "reply", rename_all = "snake_case")]
pub enum InteractionReply {
    Approval(ApprovalDecision),
    UserAnswers(Vec<QuestionAnswer>),
}

pub(crate) type InteractionCallback = std::sync::Arc<
    dyn Fn(InteractionRequest) -> BoxFuture<'static, InteractionReply> + Send + Sync,
>;

fn invalid_interaction<T>(message: &str) -> Result<T, crate::LlmError> {
    Err(invalid_interaction_error(message))
}

fn invalid_interaction_error(message: &str) -> crate::LlmError {
    crate::LlmError::InvalidToolArguments(message.into())
}
