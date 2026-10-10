#![deny(clippy::undocumented_unsafe_blocks)]

mod command;
mod web_search;
mod web_search_html;

pub use command::{
    CommandEntity, CommandError, CommandInvocation, CommandOutcome, DirectoryIdentity,
    ExecutableIdentity, SandboxFilesystemPolicy, SandboxNetworkPolicy,
    SandboxedCommandAuthorization, SandboxedCommandOptions, TerminalLogs, execute_command,
    execute_command_in_directory, execute_command_in_directory_with_arguments,
    execute_command_in_directory_with_arguments_and_options, execute_sandboxed_command,
    parse_posix_command, prepare_sandboxed_command, prepare_sandboxed_command_with_options,
};

pub use web_search::{WebSearchError, WebSearchResponse, search_target_host, web_search};
