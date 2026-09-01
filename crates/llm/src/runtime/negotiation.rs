use super::{
    ProviderRegistration,
    option_keys::{GENERATION_OPTION_KEYS, LOCAL_RUNTIME_OPTION_KEYS},
    option_validation::validate_option_values,
};
use crate::{
    CapabilitySupport, LlmError, LlmOptions, ModelCapability, OptionHandlingMode,
    ProviderCapabilities, ReasoningEffort,
};

pub(super) use super::provider_content_validation::{validate_context, validate_user_message};

pub(crate) fn negotiate_options(
    registration: &ProviderRegistration,
    options: &mut LlmOptions,
) -> Result<Vec<String>, LlmError> {
    validate_option_values(options)?;

    let provider_capabilities = registration.provider.capabilities();
    let mut warning_messages = Vec::new();

    negotiate_reasoning_options(options, &provider_capabilities, &mut warning_messages)?;
    negotiate_generation_options(options, &provider_capabilities, &mut warning_messages)?;
    negotiate_local_runtime_options(options, &provider_capabilities, &mut warning_messages)?;

    Ok(warning_messages)
}

fn negotiate_reasoning_options(
    options: &mut LlmOptions,
    provider_capabilities: &ProviderCapabilities,
    warning_messages: &mut Vec<String>,
) -> Result<(), LlmError> {
    let reasoning_effort = options.reasoning.effort;
    let reasoning_effort_support = provider_capabilities
        .reasoning_efforts
        .get(&reasoning_effort)
        .copied()
        .unwrap_or(CapabilitySupport::Unknown);
    let reasoning_capability_support =
        provider_capabilities.support_for(ModelCapability::Reasoning);
    let selected_effort_is_unsupported = reasoning_effort_support == CapabilitySupport::Unsupported;
    let reasoning_is_unavailable = reasoning_capability_support == CapabilitySupport::Unsupported;
    let effort_requires_reasoning = reasoning_effort != ReasoningEffort::None;
    let effort_is_unsupported =
        selected_effort_is_unsupported || (effort_requires_reasoning && reasoning_is_unavailable);

    if reasoning_effort != ReasoningEffort::Auto && effort_is_unsupported {
        handle_unsupported_reasoning_effort(options, warning_messages)?;
    }

    let token_budget_is_unsupported =
        options.reasoning.budget_tokens.is_some() && reasoning_is_unavailable;
    if token_budget_is_unsupported {
        handle_unsupported_option(options.handling, "reasoning token budget", warning_messages)?;
        options.reasoning.budget_tokens = None;
    }

    Ok(())
}

fn handle_unsupported_reasoning_effort(
    options: &mut LlmOptions,
    warning_messages: &mut Vec<String>,
) -> Result<(), LlmError> {
    let reasoning_effort = options.reasoning.effort;

    if options.handling == OptionHandlingMode::Strict {
        return Err(LlmError::UnsupportedEffort(format!("{reasoning_effort:?}")));
    }

    warning_messages.push(format!(
        "provider ignored unsupported reasoning effort {reasoning_effort:?}"
    ));
    options.reasoning.effort = ReasoningEffort::Auto;

    Ok(())
}

fn negotiate_generation_options(
    options: &mut LlmOptions,
    provider_capabilities: &ProviderCapabilities,
    warning_messages: &mut Vec<String>,
) -> Result<(), LlmError> {
    for option_key in GENERATION_OPTION_KEYS {
        let option_is_configured = option_key.is_present(&options.generation);
        let option_is_unsupported =
            option_key.provider_support(provider_capabilities) == CapabilitySupport::Unsupported;
        if option_is_configured && option_is_unsupported {
            handle_unsupported_option(
                options.handling,
                option_key.capability_name(),
                warning_messages,
            )?;
            option_key.clear_configured_value(&mut options.generation);
        }
    }

    Ok(())
}

fn negotiate_local_runtime_options(
    options: &mut LlmOptions,
    provider_capabilities: &ProviderCapabilities,
    warning_messages: &mut Vec<String>,
) -> Result<(), LlmError> {
    for option_key in LOCAL_RUNTIME_OPTION_KEYS {
        if !option_key.is_present(&options.local) {
            continue;
        }

        let option_is_unsupported =
            option_key.provider_support(provider_capabilities) == CapabilitySupport::Unsupported;
        let requires_different_lifecycle_phase = option_key
            .lifecycle_phase(provider_capabilities)
            .is_some_and(|phase| phase != crate::LocalOptionPhase::PerRequest);

        if option_is_unsupported || requires_different_lifecycle_phase {
            handle_unsupported_option(
                options.handling,
                option_key.capability_name(),
                warning_messages,
            )?;
            option_key.clear_configured_value(&mut options.local);
        }
    }

    Ok(())
}

fn handle_unsupported_option(
    option_handling_mode: OptionHandlingMode,
    option_name: &str,
    warning_messages: &mut Vec<String>,
) -> Result<(), LlmError> {
    match option_handling_mode {
        OptionHandlingMode::Strict => Err(LlmError::UnsupportedOption(option_name.into())),
        OptionHandlingMode::BestEffort => {
            warning_messages.push(format!("provider ignored unsupported option {option_name}"));

            Ok(())
        }
    }
}
