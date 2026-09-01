use super::RequestBuilder;
use crate::{AvailabilityReport, AvailabilityState, LlmError, ModelInfo, ModelScope};

impl<State> RequestBuilder<State> {
    pub async fn list_models(self) -> Result<Vec<ModelInfo>, LlmError> {
        self.validate()?;
        let provider_id = self.provider_id()?.clone();
        let models = self
            .runtime()?
            .provider_registration(&provider_id)?
            .provider
            .list_models(ModelScope::All)
            .await?;

        for model_information in &models {
            if model_information.provider != provider_id {
                return Err(LlmError::ProviderProtocol(format!(
                    "model {} was listed for the wrong provider",
                    model_information.id
                )));
            }
        }

        Ok(models)
    }

    pub async fn get_all_models(self) -> Result<Vec<ModelInfo>, LlmError> {
        self.list_models().await
    }

    pub async fn get_info(self) -> Result<ModelInfo, LlmError> {
        self.validate()?;
        let provider_id = self.provider_id()?.clone();
        let (registration, model_id) = self
            .runtime()?
            .resolve_selected_or_default_model(&provider_id, self.data.model.as_ref())?;
        let model_information = registration.provider.model_info(&model_id).await?;

        if model_information.provider != provider_id || model_information.id != model_id {
            return Err(LlmError::ProviderProtocol(
                "model_info returned a different provider or model".into(),
            ));
        }

        Ok(model_information)
    }

    pub async fn availability(self) -> Result<AvailabilityReport, LlmError> {
        self.validate()?;
        let provider_id = self.provider_id()?.clone();
        let registration = self.runtime()?.provider_registration(&provider_id)?;
        let selected_model = self
            .data
            .model
            .as_ref()
            .or(registration.default_model.as_ref());
        let mut availability_report = registration.provider.availability(selected_model).await?;

        if availability_report.provider != provider_id {
            return Err(LlmError::ProviderProtocol(
                "availability returned a different provider".into(),
            ));
        }

        if let Some(model_id) = selected_model {
            if availability_report.selected_model.as_ref() != Some(model_id) {
                return Err(LlmError::ProviderProtocol(
                    "availability returned a different selected model".into(),
                ));
            }
        } else {
            availability_report.state = AvailabilityState::MissingConfiguration;
            availability_report.model = AvailabilityState::MissingConfiguration;
            availability_report.selected_model = None;
            availability_report.message.get_or_insert_with(|| {
                "no model was selected and the provider has no configured default".into()
            });
        }

        Ok(availability_report)
    }

    pub async fn is_available(self) -> Result<bool, LlmError> {
        Ok(self.availability().await?.is_ready())
    }
}
