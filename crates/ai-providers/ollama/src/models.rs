use super::{
    metadata::{NativeModelListEntry, create_model_info},
    metadata_values::{model_matches_scope, native_model_is_remote},
    provider::OllamaProvider,
};
use llm::{LlmError, ModelId, ModelInfo, ModelScope, TransportOptions};
use std::collections::{BTreeSet, HashMap};

pub(super) async fn list(
    provider: &OllamaProvider,
    scope: ModelScope,
    transport: &TransportOptions,
) -> Result<Vec<ModelInfo>, LlmError> {
    let (model_tags, running_models) = tokio::try_join!(
        provider.fetch_model_tags(transport),
        provider.fetch_running_models(transport),
    )?;

    let running_by_name = index_running_models(&running_models);
    let mut model_information = Vec::with_capacity(model_tags.len());
    let mut known_model_names = BTreeSet::new();

    for model_tag in model_tags {
        let model_identifier = ModelId::new(model_tag.name.clone())?;

        known_model_names.insert(model_tag.name.clone());

        if let Some(canonical_model_identifier) = model_tag.model.as_deref() {
            known_model_names.insert(canonical_model_identifier.to_string());
        }

        let running_model = find_running_model(&model_tag, &running_by_name);

        if !native_model_supports_chat(&model_tag) {
            continue;
        }

        if !model_matches_scope(
            scope,
            native_model_is_remote(Some(&model_tag), running_model),
            running_model.is_some(),
        ) {
            continue;
        }

        model_information.push(create_model_info(
            model_identifier,
            Some(&model_tag),
            None,
            running_model,
            &provider.provider_identifier,
        ));
    }

    append_unlisted_running_models(
        &mut model_information,
        &running_models,
        &known_model_names,
        provider,
        scope,
    )?;

    model_information.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(model_information)
}

pub(super) async fn info(
    provider: &OllamaProvider,
    selected_model: ModelId,
    transport: &TransportOptions,
) -> Result<ModelInfo, LlmError> {
    let (model_details, model_tags, running_models) = tokio::try_join!(
        provider.fetch_model_details(&selected_model, false, transport),
        provider.fetch_model_tags(transport),
        provider.fetch_running_models(transport),
    )?;

    let model_tag = select_model_entry(&model_tags, &selected_model)?.cloned();
    let running_model = select_model_entry(&running_models, &selected_model)?.cloned();

    Ok(create_model_info(
        selected_model,
        model_tag.as_ref(),
        Some(model_details),
        running_model.as_ref(),
        &provider.provider_identifier,
    ))
}

fn select_model_entry<'a>(
    model_entries: &'a [NativeModelListEntry],
    selected_model: &ModelId,
) -> Result<Option<&'a NativeModelListEntry>, LlmError> {
    let exact_matches = model_entries
        .iter()
        .filter(|model_entry| model_entry.name == selected_model.as_str())
        .collect::<Vec<_>>();

    if exact_matches.len() > 1 {
        return Err(LlmError::InvalidRequest(format!(
            "ambiguous exact Ollama model name {}",
            selected_model.as_str()
        )));
    }

    if let Some(exact_match) = exact_matches.into_iter().next() {
        return Ok(Some(exact_match));
    }

    let canonical_matches = model_entries
        .iter()
        .filter(|model_entry| model_entry.model.as_deref() == Some(selected_model.as_str()))
        .collect::<Vec<_>>();

    match canonical_matches.as_slice() {
        [] => Ok(None),
        [canonical_match] => Ok(Some(*canonical_match)),
        _ => Err(LlmError::InvalidRequest(format!(
            "ambiguous canonical Ollama model name {}",
            selected_model.as_str()
        ))),
    }
}

fn index_running_models(running_models: &[NativeModelListEntry]) -> HashMap<&str, &NativeModelListEntry> {
    let mut running_by_name = HashMap::new();

    for running_model in running_models {
        running_by_name.insert(running_model.name.as_str(), running_model);

        if let Some(model_identifier) = running_model.model.as_deref() {
            running_by_name.insert(model_identifier, running_model);
        }
    }

    running_by_name
}

fn find_running_model<'a>(
    model_tag: &NativeModelListEntry,
    running_by_name: &HashMap<&'a str, &'a NativeModelListEntry>,
) -> Option<&'a NativeModelListEntry> {
    running_by_name.get(model_tag.name.as_str()).copied().or_else(|| {
        model_tag
            .model
            .as_deref()
            .and_then(|model_identifier| running_by_name.get(model_identifier))
            .copied()
    })
}

fn append_unlisted_running_models(
    model_information: &mut Vec<ModelInfo>,
    running_models: &[NativeModelListEntry],
    known_model_names: &BTreeSet<String>,
    provider: &OllamaProvider,
    scope: ModelScope,
) -> Result<(), LlmError> {
    for running_model in running_models {
        if known_model_names.contains(&running_model.name)
            || running_model
                .model
                .as_deref()
                .is_some_and(|model_identifier| known_model_names.contains(model_identifier))
        {
            continue;
        }

        let model_identifier = ModelId::new(running_model.name.clone())?;

        if !native_model_supports_chat(running_model) {
            continue;
        }

        if !model_matches_scope(scope, native_model_is_remote(None, Some(running_model)), true) {
            continue;
        }

        model_information.push(create_model_info(
            model_identifier,
            None,
            None,
            Some(running_model),
            &provider.provider_identifier,
        ));
    }

    Ok(())
}

pub(crate) fn native_model_supports_chat(model: &NativeModelListEntry) -> bool {
    let raw_model = model.raw_value();
    let Some(capabilities) = raw_model.get("capabilities") else {
        return true;
    };
    let Some(capabilities) = capabilities.as_array() else {
        return false;
    };
    capabilities.iter().any(|capability| {
        capability
            .as_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("completion"))
    })
}
