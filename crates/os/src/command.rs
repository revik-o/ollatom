use serde::{Deserialize, Serialize};

#[cfg(not(windows))]
use std::process::Stdio;

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
    task::{Context, Poll},
};
use thiserror::Error;

#[cfg(not(windows))]
use tokio::process::{Child, Command};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::{Semaphore, mpsc, oneshot, watch},
    task::JoinHandle,
};

#[cfg(windows)]
#[path = "command_windows.rs"]
pub(crate) mod command_windows;

mod api;
mod authorization;
mod environment;
mod execution;
mod identity;
mod monitor;
mod output;
mod process;
mod sandbox;

pub use api::{
    execute_command, execute_command_in_directory, execute_command_in_directory_with_arguments,
    execute_command_in_directory_with_arguments_and_options, parse_posix_command,
    prepare_sandboxed_command, prepare_sandboxed_command_with_options,
};
use authorization::{
    open_authorized_executable, open_authorized_workspace, resolve_trusted_executable,
    validate_authorization_identity, validate_pinned_executable_path,
    validate_pinned_workspace_path,
};
use environment::build_sanitized_environment;

#[cfg(windows)]
use environment::is_safe_environment_name;

pub use execution::execute_sandboxed_command;

#[cfg(windows)]
use identity::windows_file_identity;

use identity::{directory_identity, display_command, executable_identity};
use monitor::monitor_command;
use output::{
    append_output, command_semaphore, join_output_reader, output_drain_deadline,
    spawn_output_reader,
};

#[cfg(not(windows))]
use process::{configure_process_group, terminate_process_tree, terminate_remaining_descendants};

use process::{default_system_directories, trusted_executable_directories};
use sandbox::{build_sandbox_process, ensure_sandbox_backend_available};

const MAXIMUM_CONCURRENT_COMMAND_COUNT: usize = 4;
const MAXIMUM_STANDARD_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_STANDARD_ERROR_BYTES: usize = 8 * 1024 * 1024;

#[cfg(not(windows))]
const MAXIMUM_COMBINED_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

const OUTPUT_BUFFER_BYTE_COUNT: usize = 8192;
const OUTPUT_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandError {
    #[error("command must contain an executable")]
    EmptyCommand,
    #[error("command arguments could not be parsed")]
    InvalidCommand,
    #[error("command execution requires a Tokio runtime")]
    MissingRuntime,
    #[error("command sandbox is unavailable: {0}")]
    SandboxUnavailable(String),
    #[error("command authorization is invalid: {0}")]
    InvalidAuthorization(String),
    #[error("command could not be started: {0}")]
    Spawn(String),
    #[error("command output failed: {0}")]
    Output(String),
    #[error("command output exceeded the configured limit")]
    OutputLimitExceeded,
    #[error("command completion was disconnected")]
    CompletionDisconnected,
    #[error("command termination failed: {0}")]
    Termination(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct TerminalLogs {
    pub standard_output: String,
    pub standard_error: String,
    pub combined_output: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct CommandOutcome {
    pub command: String,
    pub exit_code: Option<i32>,
    pub succeeded: bool,
    pub terminated: bool,
    pub terminal_logs: TerminalLogs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct CommandInvocation {
    pub executable: OsString,
    pub arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutableIdentity {
    Unix {
        device_identifier: u64,
        inode_identifier: u64,
    },
    Windows {
        volume_identifier: Vec<u8>,
        file_identifier: [u8; 16],
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectoryIdentity {
    Unix {
        device_identifier: u64,
        inode_identifier: u64,
    },
    Windows {
        volume_identifier: Vec<u8>,
        file_identifier: [u8; 16],
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxFilesystemPolicy {
    pub workspace_directory: PathBuf,
    pub read_only_system_directories: Vec<PathBuf>,
    pub executable_search_directories: Vec<PathBuf>,
    pub additional_read_only_directories: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SandboxNetworkPolicy {
    #[default]
    Denied,
    Allowed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct SandboxedCommandOptions {
    pub network_policy: SandboxNetworkPolicy,
    pub executable_search_directories: Vec<PathBuf>,
    pub additional_read_only_directories: Vec<PathBuf>,
    pub environment_variables: BTreeMap<OsString, OsString>,
}

impl SandboxedCommandOptions {
    #[must_use]
    pub fn network_policy(mut self, network_policy: SandboxNetworkPolicy) -> Self {
        self.network_policy = network_policy;
        self
    }

    #[must_use]
    pub fn executable_search_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.executable_search_directories.push(directory.into());
        self
    }

    #[must_use]
    pub fn additional_read_only_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.additional_read_only_directories.push(directory.into());
        self
    }

    pub fn environment_variable(
        mut self,
        name: impl Into<OsString>,
        value: impl Into<OsString>,
    ) -> Self {
        self.environment_variables.insert(name.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxedCommandAuthorization {
    pub executable_path: PathBuf,
    pub executable_identity: ExecutableIdentity,
    pub arguments: Vec<OsString>,
    pub working_directory_identity: DirectoryIdentity,
    pub environment: BTreeMap<OsString, OsString>,
    pub filesystem_policy: SandboxFilesystemPolicy,
    pub network_policy: SandboxNetworkPolicy,
}

#[must_use]
pub struct CommandEntity {
    command: String,
    terminal_logs: Arc<Mutex<TerminalLogs>>,
    termination_sender: watch::Sender<bool>,
    completion: Option<oneshot::Receiver<Result<CommandOutcome, CommandError>>>,
}

#[derive(Clone, Copy)]
enum OutputDestination {
    StandardOutput,
    StandardError,
}

enum OutputMessage {
    Chunk {
        destination: OutputDestination,
        bytes: Vec<u8>,
    },
    Closed,
    Failed(String),
}

impl CommandOutcome {
    pub fn terminal_logs(&self) -> &TerminalLogs {
        &self.terminal_logs
    }
}
