use super::*;

pub(crate) fn quote_qualified_identifier(
    identifier: &str,
    allow_wildcard: bool,
) -> InfrastructureResult<String> {
    let segments = identifier.split('.').collect::<Vec<_>>();

    if segments.is_empty() || segments.len() > 2 {
        return Err(sql_builder_error(format!(
            "SQL identifier '{identifier}' is invalid"
        )));
    }

    segments
        .iter()
        .enumerate()
        .map(|(segment_index, segment)| {
            if allow_wildcard && *segment == "*" && segment_index == segments.len() - 1 {
                Ok("*".to_owned())
            } else {
                quote_identifier_segment(segment)
            }
        })
        .collect::<InfrastructureResult<Vec<_>>>()
        .map(|quoted_segments| quoted_segments.join("."))
}

pub(crate) fn quote_identifier_segment(identifier: &str) -> InfrastructureResult<String> {
    let mut characters = identifier.chars();
    let first_character = characters
        .next()
        .ok_or_else(|| sql_builder_error("SQL identifier segment must not be empty"))?;

    if !(first_character == '_' || first_character.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(sql_builder_error(format!(
            "SQL identifier segment '{identifier}' is invalid"
        )));
    }

    Ok(format!("\"{identifier}\""))
}

pub(crate) fn validate_single_statement_fragment(fragment: String) -> InfrastructureResult<String> {
    if fragment.trim().is_empty() || fragment.contains(';') {
        return Err(sql_builder_error(
            "SQL fragment must be nonblank and must not contain semicolons",
        ));
    }

    Ok(fragment)
}
