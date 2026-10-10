#![cfg(windows)]

// Compile the production modules in this test target so tests can use crate-private internals.
include!("../src/lib.rs");

mod unit;
