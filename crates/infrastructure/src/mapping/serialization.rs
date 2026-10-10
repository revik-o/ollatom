use crate::{InfrastructureError, InfrastructureErrorKind, InfrastructureResult};
use serde_json::Value;

pub(crate) fn serialize_json(value: &Value) -> InfrastructureResult<String> {
    serde_json::to_string(value).map_err(|source| {
        InfrastructureError::new(
            InfrastructureErrorKind::DatabaseOperationFailed,
            format!("failed to serialize JSON value: {source}"),
        )
    })
}
