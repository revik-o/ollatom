use crate::{FilesystemAccess, TrustedFolder};
use std::path::{Component, Path};

pub(crate) fn folder_permits(
    trusted_folder: &TrustedFolder,
    path: &Path,
    access: FilesystemAccess,
) -> bool {
    if access == FilesystemAccess::Delete && !trusted_folder.allow_delete {
        return false;
    }

    if !trusted_folder.path.is_absolute() || !path.is_absolute() {
        return false;
    }

    let traverses_parent_directory = path
        .components()
        .any(|component| matches!(component, Component::ParentDir));

    if traverses_parent_directory {
        return false;
    }

    path.starts_with(&trusted_folder.path)
}

pub(crate) fn normalize_command(program: &str, arguments: &[String]) -> String {
    std::iter::once(program)
        .chain(arguments.iter().map(String::as_str))
        .map(normalize_command_argument)
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_command_argument(argument: &str) -> String {
    if can_remain_unquoted(argument) {
        argument.into()
    } else {
        serde_json::to_string(argument).expect("serializing a command argument cannot fail")
    }
}

fn can_remain_unquoted(argument: &str) -> bool {
    !argument.is_empty()
        && argument
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_./:-=+@".contains(character))
}
