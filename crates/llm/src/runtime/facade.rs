use super::LlmRuntime;
use crate::{IntoProviderId, LlmError, RequestBuilder};
use std::sync::{Arc, OnceLock};

static GLOBAL_RUNTIME: OnceLock<Arc<LlmRuntime>> = OnceLock::new();

pub struct Llm;

pub type LLM = Llm;

impl Llm {
    pub fn install_global(runtime: impl Into<Arc<LlmRuntime>>) -> Result<(), LlmError> {
        GLOBAL_RUNTIME
            .set(runtime.into())
            .map_err(|_| LlmError::GlobalRuntimeAlreadyInstalled)
    }

    pub fn init<P: IntoProviderId>(provider: P) -> RequestBuilder<crate::MissingUserMessage> {
        RequestBuilder::new(
            GLOBAL_RUNTIME.get().map(|runtime| runtime.as_ref().clone()),
            provider,
        )
    }
}
