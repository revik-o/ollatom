#![cfg(windows)]

// Compile the production modules in this test target so tests can use crate-private internals.
#[path = "../src/command.rs"]
mod command;
pub use command::*;

mod unit;
