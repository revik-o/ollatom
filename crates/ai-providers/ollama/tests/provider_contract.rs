mod support;

use llm::{
    AvailabilityState, BuiltInProvider, CapabilitySupport, ContentBlock, GenerationOptions,
    LlmOptions, LlmProvider, LlmRunOutcome, LocalRuntimeOptions, ModelCapability, TransportOptions,
    UserMessage,
};
use ollama::{DEFAULT_OLLAMA_ENDPOINT, OllamaProvider};
use serde_json::json;
use std::time::Duration;
use support::{ScriptedResponse, ScriptedServer};

#[path = "provider_contract/native_requests.rs"]
mod native_requests;
#[path = "provider_contract/provider_discovery.rs"]
mod provider_discovery;
#[path = "provider_contract/stream_timeout.rs"]
mod stream_timeout;
