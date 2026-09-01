use crate::{
    CapabilitySupport, GenerationOptions, LocalOptionPhase, LocalRuntimeOptions,
    ProviderCapabilities,
};

#[derive(Clone, Copy)]
pub(super) enum GenerationOptionKey {
    MaximumOutputTokens,
    Temperature,
    TopProbability,
    TopCandidates,
    RepeatPenalty,
    StopSequences,
}

pub(super) const GENERATION_OPTION_KEYS: [GenerationOptionKey; 6] = [
    GenerationOptionKey::MaximumOutputTokens,
    GenerationOptionKey::Temperature,
    GenerationOptionKey::TopProbability,
    GenerationOptionKey::TopCandidates,
    GenerationOptionKey::RepeatPenalty,
    GenerationOptionKey::StopSequences,
];

impl GenerationOptionKey {
    pub(super) const fn capability_name(self) -> &'static str {
        match self {
            Self::MaximumOutputTokens => "max_output_tokens",
            Self::Temperature => "temperature",
            Self::TopProbability => "top_p",
            Self::TopCandidates => "top_k",
            Self::RepeatPenalty => "repeat_penalty",
            Self::StopSequences => "stop_sequences",
        }
    }

    pub(super) fn is_present(self, options: &GenerationOptions) -> bool {
        match self {
            Self::MaximumOutputTokens => options.max_output_tokens.is_some(),
            Self::Temperature => options.temperature.is_some(),
            Self::TopProbability => options.top_p.is_some(),
            Self::TopCandidates => options.top_k.is_some(),
            Self::RepeatPenalty => options.repeat_penalty.is_some(),
            Self::StopSequences => options.stop_sequences.is_some(),
        }
    }

    pub(super) fn provider_support(self, capabilities: &ProviderCapabilities) -> CapabilitySupport {
        capabilities
            .generation_options
            .get(self.capability_name())
            .copied()
            .unwrap_or(CapabilitySupport::Unknown)
    }

    pub(super) fn clear_configured_value(self, options: &mut GenerationOptions) {
        match self {
            Self::MaximumOutputTokens => options.max_output_tokens = None,
            Self::Temperature => options.temperature = None,
            Self::TopProbability => options.top_p = None,
            Self::TopCandidates => options.top_k = None,
            Self::RepeatPenalty => options.repeat_penalty = None,
            Self::StopSequences => options.stop_sequences = None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum LocalRuntimeOptionKey {
    ContextSize,
    EvaluationBatchSize,
    ThreadCount,
    KeepAlive,
}

pub(super) const LOCAL_RUNTIME_OPTION_KEYS: [LocalRuntimeOptionKey; 4] = [
    LocalRuntimeOptionKey::ContextSize,
    LocalRuntimeOptionKey::EvaluationBatchSize,
    LocalRuntimeOptionKey::ThreadCount,
    LocalRuntimeOptionKey::KeepAlive,
];

impl LocalRuntimeOptionKey {
    pub(super) const fn capability_name(self) -> &'static str {
        match self {
            Self::ContextSize => "context_size",
            Self::EvaluationBatchSize => "evaluation_batch_size",
            Self::ThreadCount => "threads",
            Self::KeepAlive => "keep_alive",
        }
    }

    pub(super) fn is_present(self, options: &LocalRuntimeOptions) -> bool {
        match self {
            Self::ContextSize => options.context_size.is_some(),
            Self::EvaluationBatchSize => options.evaluation_batch_size.is_some(),
            Self::ThreadCount => options.threads.is_some(),
            Self::KeepAlive => options.keep_alive_seconds.is_some(),
        }
    }

    pub(super) fn provider_support(self, capabilities: &ProviderCapabilities) -> CapabilitySupport {
        capabilities
            .local_runtime_options
            .get(self.capability_name())
            .copied()
            .unwrap_or(CapabilitySupport::Unknown)
    }

    pub(super) fn lifecycle_phase(
        self,
        capabilities: &ProviderCapabilities,
    ) -> Option<LocalOptionPhase> {
        capabilities
            .local_option_phases
            .get(self.capability_name())
            .copied()
    }

    pub(super) fn clear_configured_value(self, options: &mut LocalRuntimeOptions) {
        match self {
            Self::ContextSize => options.context_size = None,
            Self::EvaluationBatchSize => options.evaluation_batch_size = None,
            Self::ThreadCount => options.threads = None,
            Self::KeepAlive => options.keep_alive_seconds = None,
        }
    }
}
