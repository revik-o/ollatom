mod support;

use infrastructure::{
    ChatUpdateOptions, CommandActionDetails, ExecutionSettingsUpdate, InfrastructureErrorKind, LlmActionDetails,
    LlmActionInput, LlmActionStatus, LlmActionStatusEventInput, LlmMessageState, MessageRoleMetadata,
};
use std::collections::BTreeMap;
use support::{create_initialized_test_infrastructure, create_project_with_chat};

fn command_action_details() -> LlmActionDetails {
    LlmActionDetails::Command(CommandActionDetails {
        command_text: "echo answer".to_owned(),
        working_directory: "/tmp".to_owned(),
        environment: BTreeMap::new(),
    })
}

fn status_event(status: LlmActionStatus) -> LlmActionStatusEventInput {
    LlmActionStatusEventInput { status, payload: None }
}

#[path = "typed_persistence/action_transitions.rs"]
mod action_transitions;
#[path = "typed_persistence/chat_transactions.rs"]
mod chat_transactions;
