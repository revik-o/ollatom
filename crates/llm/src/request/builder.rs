use super::RequestData;
use crate::{
    AllowedPermissions, ApprovalDecision, ApprovalRequest, ConversationMessage, IntoModelId,
    IntoProviderId, LlmError, LlmOptions, LlmRun, ProviderId, ReasoningEffort, RunLimits,
    RunPolicy, TrustedFolder, UserMessage,
};
use std::{future::Future, marker::PhantomData, path::PathBuf, sync::Arc};

pub struct MissingUserMessage;

pub struct HasUserMessage;

#[must_use]
pub struct RequestBuilder<State> {
    pub(crate) data: RequestData,
    state: PhantomData<State>,
}

impl<State> RequestBuilder<State> {
    pub(crate) fn new(runtime: Option<crate::LlmRuntime>, provider: impl IntoProviderId) -> Self {
        let (provider, validation_error) = match provider.into_provider_id() {
            Ok(provider) => (Some(provider), None),
            Err(error) => (None, Some(error)),
        };

        Self {
            data: RequestData {
                runtime,
                provider,
                model: None,
                options: LlmOptions::default(),
                explicit_effort: None,
                system_prompt: None,
                context: Vec::new(),
                user_message: None,
                selected_tools: Default::default(),
                policy: RunPolicy::default(),
                callbacks: Default::default(),
                interaction_callback: None,
                limits: RunLimits::default(),
                validation_error,
            },
            state: PhantomData,
        }
    }

    pub fn model(mut self, model: impl IntoModelId) -> Self {
        match model.into_model_id() {
            Ok(model) => self.data.model = Some(model),
            Err(error) => self.set_error(error),
        }
        self
    }

    pub fn effort<Effort>(mut self, effort: Effort) -> Self
    where
        Effort: TryInto<ReasoningEffort>,
        Effort::Error: std::fmt::Display,
    {
        match effort.try_into() {
            Ok(effort) => self.data.explicit_effort = Some(effort),
            Err(error) => self.set_error(LlmError::UnsupportedEffort(error.to_string())),
        }
        self
    }

    pub fn options(mut self, options: LlmOptions) -> Self {
        self.data.options = options;
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.data.system_prompt = Some(prompt.into());
        self
    }

    pub fn context(mut self, context: impl IntoIterator<Item = ConversationMessage>) -> Self {
        self.data.context = context.into_iter().collect();
        self
    }

    pub fn tools<Names, Name>(mut self, names: Names) -> Self
    where
        Names: IntoIterator<Item = Name>,
        Name: Into<String>,
    {
        self.data.selected_tools = names.into_iter().map(Into::into).collect();
        self
    }

    pub fn trusted_folders<Paths, FolderPath>(mut self, paths: Paths) -> Self
    where
        Paths: IntoIterator<Item = FolderPath>,
        FolderPath: Into<PathBuf>,
    {
        for path in paths {
            self.data
                .policy
                .add_trusted_folder(TrustedFolder::standard(path));
        }
        self
    }

    pub fn trusted_folder_grants(
        mut self,
        folders: impl IntoIterator<Item = TrustedFolder>,
    ) -> Self {
        for folder in folders {
            self.data.policy.add_trusted_folder(folder);
        }
        self
    }

    pub fn trusted_commands<Patterns, Pattern>(mut self, patterns: Patterns) -> Self
    where
        Patterns: IntoIterator<Item = Pattern>,
        Pattern: Into<String>,
    {
        for pattern in patterns {
            match self.data.policy.add_trusted_command(pattern) {
                Ok(()) => {}
                Err(error) => self.set_error(error),
            }
        }

        self
    }
    pub fn allowed(mut self, permissions: AllowedPermissions) -> Self {
        self.data.policy.allow_permissions(permissions);
        self
    }

    pub fn deny_untrusted(mut self) -> Self {
        self.data.policy.set_deny_untrusted();
        self
    }

    pub fn stream_approval_requests(mut self) -> Self {
        self.data.policy.set_stream_approvals();
        self
    }
    pub fn limits(mut self, limits: RunLimits) -> Self {
        self.data.limits = limits;
        self
    }

    pub fn on_approval_request<Callback, CallbackFuture>(mut self, callback: Callback) -> Self
    where
        Callback: Fn(ApprovalRequest) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = ApprovalDecision> + Send + 'static,
    {
        self.data
            .policy
            .set_approval_handler(Arc::new(move |request| Box::pin(callback(request))));
        self
    }

    pub(crate) fn runtime(&self) -> Result<&crate::LlmRuntime, LlmError> {
        self.data.runtime.as_ref().ok_or(LlmError::MissingRuntime)
    }

    pub(crate) fn validate(&self) -> Result<(), LlmError> {
        if let Some(error) = &self.data.validation_error {
            return Err(error.clone());
        }

        self.provider_id()?;

        Ok(())
    }

    pub(crate) fn provider_id(&self) -> Result<&ProviderId, LlmError> {
        self.data.provider.as_ref().ok_or_else(|| {
            self.data.validation_error.clone().unwrap_or_else(|| {
                LlmError::InvalidRequest("request does not contain a provider".into())
            })
        })
    }

    fn set_error(&mut self, error: LlmError) {
        if self.data.validation_error.is_none() {
            self.data.validation_error = Some(error);
        }
    }
}

impl RequestBuilder<MissingUserMessage> {
    pub fn user_message(
        mut self,
        message: impl Into<UserMessage>,
    ) -> RequestBuilder<HasUserMessage> {
        self.data.user_message = Some(message.into());

        RequestBuilder {
            data: self.data,
            state: PhantomData,
        }
    }
}
impl RequestBuilder<HasUserMessage> {
    pub fn send(self) -> LlmRun {
        crate::runtime::execution::start_run(self.data)
    }
}
