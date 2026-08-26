use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SqlColumn {
    pub name: String,
    pub value: SqlValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SqlRow {
    pub(crate) columns: Vec<SqlColumn>,
}

impl SqlRow {
    pub fn columns(&self) -> &[SqlColumn] {
        &self.columns
    }

    pub fn try_get<ValueType>(&self, column_name: &str) -> InfrastructureResult<ValueType>
    where
        ValueType: TryFromSqlValue,
    {
        let matching_columns = self
            .columns
            .iter()
            .filter(|column| column.name == column_name)
            .collect::<Vec<_>>();

        match matching_columns.as_slice() {
            [] => Err(sql_builder_error(format!(
                "SQL row does not contain column '{column_name}'"
            ))),
            [column] => ValueType::try_from_sql_value(&column.value),
            _ => Err(sql_builder_error(format!(
                "SQL row contains multiple columns named '{column_name}'"
            ))),
        }
    }

    pub fn try_get_index<ValueType>(&self, column_index: usize) -> InfrastructureResult<ValueType>
    where
        ValueType: TryFromSqlValue,
    {
        let column = self.columns.get(column_index).ok_or_else(|| {
            sql_builder_error(format!("SQL row does not contain index {column_index}"))
        })?;
        ValueType::try_from_sql_value(&column.value)
    }

    pub fn to_map(&self) -> InfrastructureResult<BTreeMap<String, SqlValue>> {
        let mut values_by_column_name = BTreeMap::new();

        for column in &self.columns {
            if values_by_column_name
                .insert(column.name.clone(), column.value.clone())
                .is_some()
            {
                return Err(sql_builder_error(format!(
                    "SQL row contains multiple columns named '{}'",
                    column.name
                )));
            }
        }

        Ok(values_by_column_name)
    }

    pub fn to_json_object(&self) -> InfrastructureResult<Value> {
        let mut json_object = Map::new();

        for (column_name, column_value) in self.to_map()? {
            json_object.insert(column_name, sql_value_to_json(column_value)?);
        }

        Ok(Value::Object(json_object))
    }
}

pub trait TryFromSqlValue: Sized {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self>;
}

impl TryFromSqlValue for SqlValue {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        Ok(value.clone())
    }
}

impl TryFromSqlValue for i64 {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Integer(value) => Ok(*value),
            _ => Err(unexpected_sql_value("integer", value)),
        }
    }
}

impl TryFromSqlValue for f64 {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Real(value) => Ok(*value),
            SqlValue::Integer(value) => Ok(*value as f64),
            _ => Err(unexpected_sql_value("real", value)),
        }
    }
}

impl TryFromSqlValue for bool {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Integer(0) => Ok(false),
            SqlValue::Integer(1) => Ok(true),
            _ => Err(unexpected_sql_value("boolean integer", value)),
        }
    }
}

impl TryFromSqlValue for String {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Text(value) => Ok(value.clone()),
            _ => Err(unexpected_sql_value("text", value)),
        }
    }
}

impl TryFromSqlValue for Vec<u8> {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Blob(value) => Ok(value.clone()),
            _ => Err(unexpected_sql_value("blob", value)),
        }
    }
}

impl TryFromSqlValue for Uuid {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Blob(value) => Uuid::from_slice(value).map_err(|source| {
                sql_builder_error(format!("failed to decode UUID SQL value: {source}"))
            }),
            _ => Err(unexpected_sql_value("UUID blob", value)),
        }
    }
}

impl TryFromSqlValue for OffsetDateTime {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Text(value) => parse_timestamp(value),
            _ => Err(unexpected_sql_value("timestamp text", value)),
        }
    }
}

impl TryFromSqlValue for Value {
    fn try_from_sql_value(value: &SqlValue) -> InfrastructureResult<Self> {
        match value {
            SqlValue::Text(value) => serde_json::from_str(value).map_err(|source| {
                sql_builder_error(format!("failed to decode JSON SQL value: {source}"))
            }),
            _ => Err(unexpected_sql_value("JSON text", value)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SqlMutationResult {
    pub rows_affected: u64,
    pub returned_rows: Vec<SqlRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqlSortDirection {
    Ascending,
    Descending,
}
