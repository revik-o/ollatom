use crate::{ModelId, ProviderId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmError {
    InvalidProvider(String),
    InvalidModel(String),
    MissingRuntime,
    GlobalRuntimeAlreadyInstalled,
    ProviderNotRegistered(ProviderId),
    DuplicateProvider(ProviderId),
    ModelRequired(ProviderId),
    ModelNotFound(ModelId),
    Unavailable(String),
    UnsupportedEffort(String),
    UnsupportedOption(String),
    InvalidRequest(String),
    InvalidToolDefinition(String),
    InvalidToolArguments(String),
    ToolProtocol(String),
    ProviderProtocol(String),
    InteractionNotFound(u64),
    InteractionAlreadyResolved(u64),
    Authorization(String),
    EventSink(String),
    LoopLimit(String),
    Timeout(String),
    Cancelled,
    Provider(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProvider(value) => write!(formatter, "invalid provider: {value}"),
            Self::InvalidModel(value) => write!(formatter, "invalid model: {value}"),
            Self::MissingRuntime => formatter.write_str("no global LLM runtime is installed"),
            Self::GlobalRuntimeAlreadyInstalled => {
                formatter.write_str("the global LLM runtime is already installed")
            }
            Self::ProviderNotRegistered(provider_id) => {
                write!(formatter, "provider is not registered: {provider_id}")
            }
            Self::DuplicateProvider(provider_id) => {
                write!(formatter, "provider is registered twice: {provider_id}")
            }
            Self::ModelRequired(provider_id) => write!(
                formatter,
                "provider {provider_id} has no configured default model"
            ),
            Self::ModelNotFound(model_id) => {
                write!(formatter, "model was not found: {model_id}")
            }
            Self::Unavailable(message) => write!(formatter, "provider is unavailable: {message}"),
            Self::UnsupportedEffort(value) => {
                write!(formatter, "unsupported reasoning effort: {value}")
            }
            Self::UnsupportedOption(option_name) => {
                write!(formatter, "unsupported option: {option_name}")
            }
            Self::InvalidRequest(message) => write!(formatter, "invalid request: {message}"),
            Self::InvalidToolDefinition(message) => {
                write!(formatter, "invalid tool definition: {message}")
            }
            Self::InvalidToolArguments(message) => {
                write!(formatter, "invalid tool arguments: {message}")
            }
            Self::ToolProtocol(message) => write!(formatter, "tool protocol error: {message}"),
            Self::ProviderProtocol(message) => {
                write!(formatter, "provider protocol error: {message}")
            }
            Self::InteractionNotFound(interaction_id) => {
                write!(formatter, "interaction was not found: {interaction_id}")
            }
            Self::InteractionAlreadyResolved(interaction_id) => {
                write!(
                    formatter,
                    "interaction was already resolved: {interaction_id}"
                )
            }
            Self::Authorization(message) => write!(formatter, "authorization failed: {message}"),
            Self::EventSink(message) => write!(formatter, "event sink failed: {message}"),
            Self::LoopLimit(message) => {
                write!(formatter, "agent loop limit reached: {message}")
            }
            Self::Timeout(message) => write!(formatter, "operation timed out: {message}"),
            Self::Cancelled => formatter.write_str("run was cancelled"),
            Self::Provider(message) => write!(formatter, "provider failed: {message}"),
        }
    }
}

impl std::error::Error for LlmError {}
