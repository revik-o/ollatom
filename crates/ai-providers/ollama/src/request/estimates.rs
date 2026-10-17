use super::super::metadata_values::{first_unsigned_integer, model_info_value};
use llm::{LlmError, ModelId, ProviderRunRequest};
use serde::Serialize;
use serde_json::Value;
use std::io::Write;

const CONTEXT_OPTIMIZATION_PRESSURE_NUMERATOR: u64 = 9;
const CONTEXT_OPTIMIZATION_PRESSURE_DENOMINATOR: u64 = 10;

pub(crate) fn estimate_serialized_tokens<ValueType>(value: &ValueType) -> Result<u64, LlmError>
where
    ValueType: Serialize + ?Sized,
{
    let mut counting_writer = CountingWriter::default();
    serde_json::to_writer(&mut counting_writer, value).map_err(|error| LlmError::ProviderProtocol(error.to_string()))?;

    counting_writer
        .byte_count
        .checked_add(3)
        .map(|byte_count| byte_count / 4)
        .ok_or_else(|| LlmError::ProviderProtocol("token estimate overflow".into()))
}

#[derive(Default)]
struct CountingWriter {
    byte_count: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let buffer_byte_count = u64::try_from(buffer.len())
            .map_err(|_| std::io::Error::other("serialized value length is not representable"))?;

        self.byte_count = self
            .byte_count
            .checked_add(buffer_byte_count)
            .ok_or_else(|| std::io::Error::other("serialized value length overflow"))?;

        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn context_input_limit(request: &ProviderRunRequest, model_details: &Value) -> Option<u64> {
    let configured_context_length = request
        .options
        .local
        .context_size
        .map(u64::from)
        .filter(|context_size| *context_size > 0);

    let discovered_context_length = first_unsigned_integer([
        model_details.get("context_length"),
        model_details
            .get("details")
            .and_then(|value| value.get("context_length")),
        model_details
            .get("model_info")
            .and_then(|value| value.get("context_length")),
        model_details
            .get("model_info")
            .and_then(|value| model_info_value(value, "context_length")),
    ]);

    [configured_context_length, discovered_context_length]
        .into_iter()
        .flatten()
        .filter(|context_length| *context_length > 0)
        .min()
}

pub(crate) fn context_pressure_reached(observed_input_tokens: u64, estimated_tool_tokens: u64, limit: u64) -> bool {
    let pressure = observed_input_tokens.saturating_add(estimated_tool_tokens);

    pressure.saturating_mul(CONTEXT_OPTIMIZATION_PRESSURE_DENOMINATOR)
        >= limit.saturating_mul(CONTEXT_OPTIMIZATION_PRESSURE_NUMERATOR)
}

pub(crate) fn model_is_gemma_four(model: &ModelId) -> bool {
    model.as_str().to_ascii_lowercase().starts_with("gemma4")
}
