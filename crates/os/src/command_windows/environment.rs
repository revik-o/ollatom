use super::super::{CommandError, SandboxedCommandAuthorization};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(super) fn system_root_directory() -> Result<PathBuf, CommandError> {
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| CommandError::SandboxUnavailable("SystemRoot is not configured".into()))?;
    let system_root_path = std::fs::canonicalize(PathBuf::from(system_root))
        .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;
    let system_root_metadata = std::fs::symlink_metadata(&system_root_path)
        .map_err(|error| CommandError::SandboxUnavailable(error.to_string()))?;

    if !system_root_metadata.is_dir() {
        return Err(CommandError::SandboxUnavailable(
            "SystemRoot is not a directory".into(),
        ));
    }

    Ok(system_root_path)
}

pub(super) fn build_windows_environment(
    authorized_environment: &BTreeMap<OsString, OsString>,
    authorization: &SandboxedCommandAuthorization,
) -> Result<Vec<(OsString, OsString)>, CommandError> {
    let workspace_directory = authorization
        .filesystem_policy
        .workspace_directory
        .as_os_str()
        .to_owned();

    let system_root = system_root_directory()?.into_os_string();

    let system_directory = PathBuf::from(&system_root)
        .join("System32")
        .into_os_string();

    let mut executable_search_directories = vec![system_directory];

    executable_search_directories.extend(
        authorization
            .filesystem_policy
            .executable_search_directories
            .iter()
            .map(|directory| directory.as_os_str().to_owned()),
    );

    let executable_search_directories = std::env::join_paths(executable_search_directories)
        .map_err(|error| CommandError::InvalidAuthorization(error.to_string()))?;

    let mut environment = BTreeMap::from([
        (OsString::from("HOME"), workspace_directory.clone()),
        (OsString::from("PATH"), executable_search_directories),
        (OsString::from("SystemRoot"), system_root.clone()),
        (OsString::from("TEMP"), workspace_directory.clone()),
        (OsString::from("TMP"), workspace_directory.clone()),
        (OsString::from("USERPROFILE"), workspace_directory.clone()),
        (OsString::from("WINDIR"), system_root),
    ]);

    for (name, value) in authorized_environment {
        if super::super::is_safe_environment_name(name) {
            environment.insert(name.clone(), value.clone());
        }
    }

    let mut environment: Vec<_> = environment.into_iter().collect();

    environment.sort_by_cached_key(|(name, _)| name.to_string_lossy().to_ascii_lowercase());

    Ok(environment)
}

pub(super) fn build_windows_command_line(
    executable_path: &Path,
    arguments: &[OsString],
) -> Result<String, CommandError> {
    let executable = executable_path
        .to_str()
        .ok_or(CommandError::InvalidCommand)?;

    let arguments = arguments
        .iter()
        .map(|argument| argument.to_str().ok_or(CommandError::InvalidCommand))
        .collect::<Result<Vec<_>, _>>()?;

    let command_parts = std::iter::once(executable).chain(arguments);

    Ok(command_parts
        .map(quote_windows_argument)
        .collect::<Vec<_>>()
        .join(" "))
}

pub(crate) fn quote_windows_argument(argument: &str) -> String {
    let mut quoted_argument = String::with_capacity(argument.len().saturating_add(2));

    quoted_argument.push('"');

    let mut backslash_count = 0_usize;

    for character in argument.chars() {
        if character == '\\' {
            backslash_count = backslash_count.saturating_add(1);
        } else if character == '"' {
            quoted_argument
                .push_str(&"\\".repeat(backslash_count.saturating_mul(2).saturating_add(1)));
            quoted_argument.push('"');
            backslash_count = 0;
        } else {
            quoted_argument.push_str(&"\\".repeat(backslash_count));
            quoted_argument.push(character);
            backslash_count = 0;
        }
    }

    quoted_argument.push_str(&"\\".repeat(backslash_count.saturating_mul(2)));
    quoted_argument.push('"');

    quoted_argument
}
