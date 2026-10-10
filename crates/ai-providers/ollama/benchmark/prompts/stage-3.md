Use the sandboxed execute_command tool to check that the Rust toolchain is available through mise. Run these two commands separately with program `mise` and the exact arguments shown:

1. `["exec", "--", "rustc", "--version"]`
2. `["exec", "--", "cargo", "--version"]`

Both version lines must appear in the command output printed to the console. Do not edit any project files in this stage. Finish by reporting the exact `rustc` and `cargo` version lines returned by the commands.
