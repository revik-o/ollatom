use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<bool> for SqlValue {
    fn from(value: bool) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for SqlValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for SqlValue {
    fn from(value: i32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<u8> for SqlValue {
    fn from(value: u8) -> Self {
        Self::Integer(value.into())
    }
}

impl From<u32> for SqlValue {
    fn from(value: u32) -> Self {
        Self::Integer(value.into())
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl From<Uuid> for SqlValue {
    fn from(value: Uuid) -> Self {
        Self::Blob(value.as_bytes().to_vec())
    }
}

impl TryFrom<OffsetDateTime> for SqlValue {
    type Error = InfrastructureError;

    fn try_from(value: OffsetDateTime) -> Result<Self, Self::Error> {
        format_timestamp(value).map(Self::Text)
    }
}

impl TryFrom<Value> for SqlValue {
    type Error = InfrastructureError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        serde_json::to_string(&value)
            .map(Self::Text)
            .map_err(|source| {
                sql_builder_error(format!("failed to serialize JSON SQL value: {source}"))
            })
    }
}
