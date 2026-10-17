mod artifact;
mod history;
mod logging;
mod verifier;

use std::{env, path::PathBuf};

pub const MAXIMUM_UNAVAILABLE_BUILD_ATTEMPTS: usize = 5;

pub use artifact::BenchmarkArtifacts;
pub use history::ConversationHistoryRecorder;
pub use logging::{AgentStageBuildStatus, ModelToolActivity, StageLogger};
pub use verifier::verify_independent_build;

pub fn node_toolchain_command_options(
    network_policy: os::SandboxNetworkPolicy,
) -> std::io::Result<os::SandboxedCommandOptions> {
    let path = std::env::var_os("PATH").ok_or_else(|| std::io::Error::other("benchmark PATH is not configured"))?;

    for binary_directory in std::env::split_paths(&path) {
        if !binary_directory.join("node").is_file() || !binary_directory.join("npm").is_file() {
            continue;
        }

        let binary_directory = std::fs::canonicalize(binary_directory)?;
        let installation_directory = binary_directory
            .parent()
            .ok_or_else(|| std::io::Error::other("Node.js installation path is invalid"))?
            .to_path_buf();

        return Ok(os::SandboxedCommandOptions::default()
            .network_policy(network_policy)
            .executable_search_directory(binary_directory)
            .additional_read_only_directory(installation_directory));
    }

    Err(std::io::Error::other(
        "benchmark requires node and npm on PATH; run it through mise exec",
    ))
}

pub fn programming_toolchain_command_options(
    network_policy: os::SandboxNetworkPolicy,
) -> std::io::Result<os::SandboxedCommandOptions> {
    let mut options = node_toolchain_command_options(network_policy)?;
    let home_directory = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    if let Some(rustup_home) = env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| home_directory.map(|directory| directory.join(".rustup")))
        .filter(|directory| directory.is_dir())
    {
        options = options
            .additional_read_only_directory(&rustup_home)
            .environment_variable("RUSTUP_HOME", rustup_home.into_os_string());
    }

    if let Some(rustup_toolchain) = env::var_os("RUSTUP_TOOLCHAIN") {
        options = options.environment_variable("RUSTUP_TOOLCHAIN", rustup_toolchain);
    }

    let cargo_binary_directory = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).find(|directory| {
            directory.join("rustup").is_file() && directory.join("rustc").is_file() && directory.join("cargo").is_file()
        })
    });

    if let Some(cargo_binary_directory) = cargo_binary_directory {
        options = options
            .executable_search_directory(&cargo_binary_directory)
            .additional_read_only_directory(cargo_binary_directory);
    }

    Ok(options)
}
