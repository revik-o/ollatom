use super::*;

#[cfg(not(windows))]
pub(super) fn build_sanitized_environment(
    requested_environment: BTreeMap<OsString, OsString>,
    trusted_search_directories: &[PathBuf],
) -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();

    environment.insert(
        OsString::from("PATH"),
        std::env::join_paths(trusted_search_directories).unwrap_or_default(),
    );

    environment.insert(OsString::from("LANG"), OsString::from("C.UTF-8"));
    environment.insert(OsString::from("LC_ALL"), OsString::from("C.UTF-8"));
    environment.insert(OsString::from("HOME"), OsString::from("/tmp/home"));
    environment.insert(OsString::from("TMPDIR"), OsString::from("/tmp"));

    for (name, value) in requested_environment {
        if is_safe_environment_name(&name) {
            environment.insert(name, value);
        }
    }

    environment
}

#[cfg(windows)]
pub(super) fn build_sanitized_environment(
    requested_environment: BTreeMap<OsString, OsString>,
    trusted_search_directories: &[PathBuf],
) -> BTreeMap<OsString, OsString> {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_default();

    let mut environment = BTreeMap::from([
        (
            OsString::from("PATH"),
            std::env::join_paths(trusted_search_directories).unwrap_or_default(),
        ),
        (OsString::from("SystemRoot"), system_root.clone()),
        (OsString::from("WINDIR"), system_root),
    ]);

    for (name, value) in requested_environment {
        if is_safe_environment_name(&name) {
            environment.insert(name, value);
        }
    }

    environment
}

pub(super) fn is_safe_environment_name(name: &OsStr) -> bool {
    matches!(
        name.to_str(),
        Some("LANG" | "LC_ALL" | "LC_CTYPE" | "RUSTUP_HOME" | "RUSTUP_TOOLCHAIN" | "TERM" | "TZ")
    )
}
