# ollatom
Open LLM space

# Clone
```bash
git clone --single-branch --depth=1 git@github.com:revik-o/ollatom.git ollatom
```

## Developer commands

Run these commands from the repository root. On Windows, replace `.sh` with
`.bat`.

```bash
# Build or test every app
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

The desktop commands require Rust, Cargo, Node.js, npm, and dependencies
installed with `npm ci` in `apps/desktop`. The TUI command wrappers are ready,
but the TUI itself must first be initialized with `apps/tui/Cargo.toml`.

Before the first desktop E2E test run, install Playwright's Chromium browser:

```bash
cd apps/desktop
npm run e2e:install
```
