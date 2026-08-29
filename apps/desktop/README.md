# ollatom desktop

## Usage

```bash
npm run start
npm run build
npm run test
```

## Production builds

Install a current Rust stable toolchain (including `cargo`) and Node/npm, then
run `npm ci` in this directory. Tauri's `build` command creates an optimized
release build; artifacts are written to the workspace `target/release/bundle/`.
The workspace release profile enables optimization level 3, fat LTO, one codegen
unit, and abort-on-panic.

From the repository root, the canonical optimized current-platform build is:

```bash
./cli/build-desktop.sh
```

Run each platform build on its native OS (or a matching CI runner):

```bash
# Windows: executable and configured installer bundles
npm run tauri -- build

# macOS: App Store-safe opaque build
npm run tauri:build:macos:store

# macOS: directly distributed build with vibrancy
npm run tauri:build:macos:direct

# Linux: AppImage/executable plus .deb and .rpm bundles
npm run tauri:build:linux
```

On Debian/Ubuntu build hosts, install `librsvg2-dev` for the GTK AppImage
plugin (`sudo apt-get install librsvg2-dev`). Tauri runs `linuxdeploy` in
extract-and-run mode, so FUSE is not required by this build.

Before distribution, sign/notarize macOS artifacts and sign Windows installers.

## Project organization

This application uses **feature-first architecture** (also known as
**folders-by-feature**): organize code by feature and ownership, not by artifact
type such as components or services.

Follow Angular's
[Organize your project by feature areas](https://angular.dev/style-guide#organize-your-project-by-feature-areas)
guidance.

Application-wide infrastructure belongs under `src/app/core`. For example,
`core/ipc` owns the low-level Tauri IPC bridge and `core/i18n` owns localization.
Keep `shared` for reusable, dependency-light UI building blocks rather than
singletons or platform integration.

Keep feature-specific IPC operations with their owning feature and have them
call `IpcService`. This keeps command names and native DTOs out of components
without growing one application-wide service into a collection of unrelated
feature APIs.
