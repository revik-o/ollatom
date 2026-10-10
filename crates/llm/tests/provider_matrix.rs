mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;

fn local_runtime_without_default() -> LlmRuntime {
    LlmRuntime::builder()
        .provider(ScriptedProvider::new(
            BuiltInProvider::LmStudio,
            ProviderBehavior::Complete,
        ))
        .build()
        .unwrap()
}

#[path = "provider_matrix/provider_capabilities.rs"]
mod provider_capabilities;
#[path = "provider_matrix/registration.rs"]
mod registration;
