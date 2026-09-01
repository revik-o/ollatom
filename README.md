# ollatom
Open LLM space

# Clone
```bash
git clone --single-branch --depth=1 git@github.com:revik-o/ollatom.git ollatom
```

## Developer commands

The repository uses [mise](https://mise.jdx.dev/) to provide the pinned Rust
and Node.js versions declared in `mise.toml`. Install mise, then prepare the
toolchains once from the repository root:

```bash
mise install
```

The command wrappers automatically run inside the mise environment, so the
Rust, Cargo, Node.js, and npm versions always come from `mise.toml`. On Windows,
replace `.sh` with `.bat`.

```bash
# Build every app, or test every crate and app
./cli/build.sh
./cli/test.sh

# Build, run, or test one app
./cli/build-desktop.sh
./cli/run-desktop.sh
./cli/test-desktop.sh
./cli/test-desktop-e2e.sh

./cli/build-tui.sh
./cli/run-tui.sh
./cli/test-tui.sh
```

The desktop commands require the dependencies installed by running `npm ci` in
`apps/desktop`; mise supplies the language toolchains. The TUI command wrappers
are ready, but the TUI itself must first be initialized with
`apps/tui/Cargo.toml`.

Before the first desktop E2E test run, install Playwright's Chromium browser:

```bash
cd apps/desktop
npm run e2e:install
```
