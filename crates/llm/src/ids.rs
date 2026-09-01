use crate::LlmError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltInProvider {
    #[serde(rename = "chatgpt")]
    ChatGpt,
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "llama-cpp")]
    LlamaCpp,
    #[serde(rename = "lm-studio")]
    LmStudio,
}

impl BuiltInProvider {
    pub const ALL: [Self; 6] = [
        Self::ChatGpt,
        Self::Claude,
        Self::Gemini,
        Self::Ollama,
        Self::LlamaCpp,
        Self::LmStudio,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
            Self::LlamaCpp => "llama-cpp",
            Self::LmStudio => "lm-studio",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, LlmError> {
        let value = value.as_ref().trim().to_ascii_lowercase();

        if !is_valid_identifier(&value) {
            return Err(LlmError::InvalidProvider(value));
        }

        Ok(Self(if value == "openai" {
            "chatgpt".into()
        } else {
            value
        }))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ProviderId {
    type Err = LlmError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<BuiltInProvider> for ProviderId {
    fn from(value: BuiltInProvider) -> Self {
        Self(value.as_str().into())
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait IntoProviderId {
    fn into_provider_id(self) -> Result<ProviderId, LlmError>;
}

impl<Provider> IntoProviderId for Provider
where
    Provider: Into<ProviderId>,
{
    fn into_provider_id(self) -> Result<ProviderId, LlmError> {
        Ok(self.into())
    }
}

impl IntoProviderId for &str {
    fn into_provider_id(self) -> Result<ProviderId, LlmError> {
        ProviderId::new(self)
    }
}

impl IntoProviderId for String {
    fn into_provider_id(self) -> Result<ProviderId, LlmError> {
        ProviderId::new(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, LlmError> {
        let value = value.as_ref().trim();

        if value.is_empty() {
            return Err(LlmError::InvalidModel(value.into()));
        }

        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ModelId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait IntoModelId {
    fn into_model_id(self) -> Result<ModelId, LlmError>;
}

impl<Model> IntoModelId for Model
where
    Model: Into<ModelId>,
{
    fn into_model_id(self) -> Result<ModelId, LlmError> {
        Ok(self.into())
    }
}

impl IntoModelId for &str {
    fn into_model_id(self) -> Result<ModelId, LlmError> {
        ModelId::new(self)
    }
}

impl IntoModelId for String {
    fn into_model_id(self) -> Result<ModelId, LlmError> {
        ModelId::new(self)
    }
}

pub(crate) fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_valid_identifier_character)
}

fn is_valid_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub u64);

impl From<u64> for RunId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
