use super::{
    metadata::{NativeModelTag, create_model_info},
    metadata_values::{model_matches_scope, native_model_is_remote},
    provider::OllamaProvider,
};
use llm::{LlmError, ModelId, ModelInfo, ModelScope};
use std::collections::{BTreeSet, HashMap};

pub(super) async fn list(
    provider: &OllamaProvider,
    scope: ModelScope,
) -> Result<Vec<ModelInfo>, LlmError> {
    let model_tags = provider.fetch_model_tags().await?;
    let running_models = provider.fetch_running_models().await?;
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
) -> Result<ModelInfo, LlmError> {
    let model_details = provider.fetch_model_details(&selected_model, true).await?;
    let model_tags = provider.fetch_model_tags().await?;
    let running_models = provider.fetch_running_models().await?;
    let model_tag = model_tags.into_iter().find(|model_tag| {
        model_tag.name == selected_model.as_str()
            || model_tag.model.as_deref() == Some(selected_model.as_str())
    });
    let running_model = running_models.into_iter().find(|running_model| {
        running_model.name == selected_model.as_str()
            || running_model.model.as_deref() == Some(selected_model.as_str())
    });

    Ok(create_model_info(
        selected_model,
        model_tag.as_ref(),
        Some(model_details),
        running_model.as_ref(),
        &provider.provider_identifier,
    ))
}

fn index_running_models<'a>(
    running_models: &'a [super::metadata::NativeRunningModel],
) -> HashMap<&'a str, &'a super::metadata::NativeRunningModel> {
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
    model_tag: &NativeModelTag,
    running_by_name: &HashMap<&'a str, &'a super::metadata::NativeRunningModel>,
) -> Option<&'a super::metadata::NativeRunningModel> {
    running_by_name
        .get(model_tag.name.as_str())
        .copied()
        .or_else(|| {
            model_tag
                .model
                .as_deref()
                .and_then(|model_identifier| running_by_name.get(model_identifier))
                .copied()
        })
}

fn append_unlisted_running_models(
    model_information: &mut Vec<ModelInfo>,
    running_models: &[super::metadata::NativeRunningModel],
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

        if !model_matches_scope(
            scope,
            native_model_is_remote(None, Some(running_model)),
            true,
        ) {
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
