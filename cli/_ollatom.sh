#!/usr/bin/env sh

set -eu

REPOSITORY_ROOT_DIRECTORY=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
CRATES_DIRECTORY="$REPOSITORY_ROOT_DIRECTORY/crates"
DESKTOP_DIRECTORY="$REPOSITORY_ROOT_DIRECTORY/apps/desktop"
TUI_MANIFEST_PATH="$REPOSITORY_ROOT_DIRECTORY/apps/tui/Cargo.toml"

if [ "${OLLATOM_MISE_EXECUTION_ACTIVE:-}" != "1" ]; then
    if ! command -v mise >/dev/null 2>&1; then
        echo "error: 'mise' is required; install it and run 'mise install' in $REPOSITORY_ROOT_DIRECTORY" >&2
        exit 1
    fi
    export OLLATOM_MISE_EXECUTION_ACTIVE=1
    exec mise exec -C "$REPOSITORY_ROOT_DIRECTORY" -- "$REPOSITORY_ROOT_DIRECTORY/cli/_ollatom.sh" "$@"
fi

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: '$1' is required but was not found on PATH" >&2
        exit 1
    fi
}

require_desktop_frontend() {
    require_command npm

    if [ ! -d "$DESKTOP_DIRECTORY/node_modules" ]; then
        echo "error: desktop dependencies are missing; run 'npm ci' in apps/desktop" >&2
        exit 1
    fi
}

require_desktop() {
    require_desktop_frontend
    require_command cargo
}

require_tui() {
    if [ ! -f "$TUI_MANIFEST_PATH" ]; then
        echo "error: the TUI app has not been initialized (missing apps/tui/Cargo.toml)" >&2
        exit 1
    fi

    require_command cargo
}

build_desktop() {
    require_desktop
    echo "==> Building desktop app"
    case "$(uname -s)" in
        Linux) (cd "$DESKTOP_DIRECTORY" && npm run tauri:build:linux) ;;
        *) (cd "$DESKTOP_DIRECTORY" && npm run tauri -- build) ;;
    esac
}

build_tui() {
    require_tui
    echo "==> Building TUI app"
    cargo build --manifest-path "$TUI_MANIFEST_PATH"
}

run_desktop() {
    require_desktop
    echo "==> Running desktop app"
    (cd "$DESKTOP_DIRECTORY" && npm run tauri -- dev)
}

run_tui() {
    require_tui
    echo "==> Running TUI app"
    cargo run --manifest-path "$TUI_MANIFEST_PATH"
}

test_desktop() {
    require_desktop
    echo "==> Testing desktop frontend"
    (cd "$DESKTOP_DIRECTORY" && npm test -- --watch=false)
    echo "==> Testing desktop Rust backend"
    cargo test --manifest-path "$DESKTOP_DIRECTORY/src-tauri/Cargo.toml"
    run_desktop_end_to_end_tests
}

run_desktop_end_to_end_tests() {
    echo "==> Testing desktop UI end to end"
    if [ "${NO_COLOR+x}" = "x" ]; then
        (
            cd "$DESKTOP_DIRECTORY"
            unset NO_COLOR
            export FORCE_COLOR=0
            npm run e2e
        )
    else
        (cd "$DESKTOP_DIRECTORY" && npm run e2e)
    fi
}

test_desktop_e2e() {
    require_desktop_frontend
    run_desktop_end_to_end_tests
}

test_crates() {
    require_command cargo
    crate_manifest_paths=$(find "$CRATES_DIRECTORY" -name Cargo.toml -print | sort)
    if [ -z "$crate_manifest_paths" ]; then
        echo "error: no Rust crates were found in $CRATES_DIRECTORY" >&2
        exit 1
    fi
    printf '%s\n' "$crate_manifest_paths" | while IFS= read -r crate_manifest_path; do
        crate_directory_name=$(basename "$(dirname "$crate_manifest_path")")
        echo "==> Testing crate $crate_directory_name"
        cargo test --manifest-path "$crate_manifest_path"
    done
}

test_tui() {
    require_tui
    echo "==> Testing TUI app"
    cargo test --manifest-path "$TUI_MANIFEST_PATH"
}

case "${1:-}" in
    build-all)
        build_desktop
        build_tui
        ;;
    build-desktop) build_desktop ;;
    build-tui) build_tui ;;
    run-desktop) run_desktop ;;
    run-tui) run_tui ;;
    test-all)
        test_crates
        test_desktop
        test_tui
        ;;
    test-desktop) test_desktop ;;
    test-desktop-e2e) test_desktop_e2e ;;
    test-crates) test_crates ;;
    test-tui) test_tui ;;
    *)
        echo "usage: $0 {build-all|build-desktop|build-tui|run-desktop|run-tui|test-all|test-desktop|test-desktop-e2e|test-crates|test-tui}" >&2
        exit 2
        ;;
esac
