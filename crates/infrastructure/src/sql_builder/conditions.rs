use super::*;

pub(crate) fn append_conditions(
    statement: &mut String,
    prefix: &str,
    conditions: Vec<SqlCondition>,
    values: &mut Vec<SqlValue>,
) {
    if conditions.is_empty() {
        return;
    }

    statement.push_str(prefix);

    for (condition_index, condition) in conditions.into_iter().enumerate() {
        if condition_index > 0 {
            statement.push(' ');
            statement.push_str(condition.connector.unwrap_or("AND"));
            statement.push(' ');
        }
        statement.push('(');
        statement.push_str(&condition.statement);
        statement.push(')');
        values.extend(condition.values);
    }
}

pub(crate) fn add_returning_columns<Columns, ColumnName>(
    returning_columns: &mut Vec<String>,
    validation_error: &mut Option<InfrastructureError>,
    column_names: Columns,
) where
    Columns: IntoIterator<Item = ColumnName>,
    ColumnName: AsRef<str>,
{
    for column_name in column_names {
        match quote_qualified_identifier(column_name.as_ref(), true) {
            Ok(column_name) => returning_columns.push(column_name),
            Err(error) => {
                if validation_error.is_none() {
                    *validation_error = Some(error);
                }
            }
        }
    }
}

pub(crate) fn append_returning_clause(statement: &mut String, returning_columns: Vec<String>) {
    if !returning_columns.is_empty() {
        statement.push_str(" RETURNING ");
        statement.push_str(&returning_columns.join(", "));
    }
}

pub(crate) fn compile_bound_condition_template<Values>(
    condition_template: &str,
    values: Values,
) -> InfrastructureResult<(String, Vec<SqlValue>)>
where
    Values: IntoIterator<Item = SqlValue>,
{
    validate_single_statement_fragment(condition_template.to_owned())?;
    let values = values.into_iter().collect::<Vec<_>>();
    let mut statement = String::new();
    let mut characters = condition_template.chars().peekable();
    let mut placeholder_count = 0;

    while let Some(character) = characters.next() {
        match (character, characters.peek().copied()) {
            ('{', Some('{')) => {
                characters.next();
                statement.push('{');
            }
            ('}', Some('}')) => {
                characters.next();
                statement.push('}');
            }
            ('{', Some('}')) => {
                characters.next();
                statement.push('?');
                placeholder_count += 1;
            }
            ('{' | '}', _) => {
                return Err(sql_builder_error(
                    "SQL condition contains an unmatched brace",
                ));
            }
            _ => statement.push(character),
        }
    }

    if placeholder_count != values.len() {
        return Err(sql_builder_error(format!(
            "SQL condition contains {placeholder_count} placeholders but received {} values",
            values.len()
        )));
    }

    Ok((statement, values))
}
