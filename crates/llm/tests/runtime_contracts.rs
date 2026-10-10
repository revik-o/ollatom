mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;

#[path = "runtime_contracts/run_lifecycle.rs"]
mod run_lifecycle;
#[path = "runtime_contracts/user_interaction.rs"]
mod user_interaction;
