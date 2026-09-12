mod command;
mod command_monitor;
mod command_output;
mod command_process;
mod web_search;
mod web_search_html;

pub use command::{
    CommandEntity, CommandError, CommandInvocation, CommandOutcome, TerminalLogs, execute_command,
    execute_command_in_directory, execute_command_in_directory_with_arguments, parse_command,
};
pub use web_search::{WebSearchError, WebSearchResponse, search_target_host, web_search};
