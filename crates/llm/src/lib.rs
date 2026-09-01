mod builtins;
mod cancellation;
mod content;
mod error;
mod events;
mod ids;
mod interaction;
mod metadata;
mod options;
mod outcome;
mod policy;
mod policy_matching;
mod provider;
mod request;
mod runtime;
mod schema;
mod subagent;
mod tools;

pub use cancellation::{StopHandle, StopToken};
pub use content::{ContentBlock, ConversationMessage, ConversationRole, UserMessage};
pub use error::LlmError;
pub use events::{
    EventCallback, EventCallbacks, RunEvent, RunEventSink, RunEventStream, SequencedEvent,
    ToolEvent,
};
pub use ids::{BuiltInProvider, IntoModelId, IntoProviderId, ModelId, ProviderId, RunId};
pub use interaction::{
    ApprovalDecision, ApprovalRequest, AskUserRequest, InteractionId, InteractionReply,
    InteractionRequest, Question, QuestionAnswer, QuestionKind,
};
pub use metadata::{
    AvailabilityReport, AvailabilityState, CapabilitySupport, ModelCapability, ModelInfo,
    ModelScope, ModelState, NormalizedValue, ProviderCapabilities, ValueProvenance,
};
pub use options::{
    ContextOptions, ContextOverflowPolicy, GenerationOptions, LlmOptions, LocalOptionPhase,
    LocalRuntimeOptions, OptionHandlingMode, ReasoningEffort, ReasoningOptions, TransportOptions,
};
pub use outcome::{
    FinishReason, LlmResponse, LlmRunOutcome, PartialResponse, ToolExecutionRecord, Usage,
    UsageSource,
};
pub use policy::{
    ALL_FILESYSTEM_ACCESS, ALL_USER_COMMANDS, AllowedPermissions, ApprovalHandler, CommandPattern,
    FilesystemAccess, RequiredCapability, RunPolicy, ToolAuthorizer, TrustedFolder,
};
pub use provider::{
    LlmProvider, ProviderRunHost, ProviderRunOutcome, ProviderRunRequest, RunLimits,
};
pub use request::{HasUserMessage, LlmRun, MissingUserMessage, RequestBuilder};
pub use runtime::{LLM, Llm, LlmRuntime, LlmRuntimeBuilder};
pub use subagent::{
    InvokeSubagentRequest, SubagentOutcome, SubagentProfile, SubagentProfileId,
    SubagentProfileRegistry, SubagentRunner,
};
pub use tools::{
    AuthorizationGrant, Tool, ToolCall, ToolDefinition, ToolFailure, ToolOutput, ToolPlan,
    ToolRegistry,
};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
