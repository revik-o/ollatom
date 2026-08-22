#!/usr/bin/env sh

set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
TUI_MANIFEST="$ROOT_DIR/apps/tui/Cargo.toml"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: '$1' is required but was not found on PATH" >&2
        exit 1
    fi
}

require_desktop_frontend() {
    require_command npm

    if [ ! -d "$DESKTOP_DIR/node_modules" ]; then
        echo "error: desktop dependencies are missing; run 'npm ci' in apps/desktop" >&2
        exit 1
    fi
}

require_desktop() {
    require_desktop_frontend
    require_command cargo
}

require_tui() {
    if [ ! -f "$TUI_MANIFEST" ]; then
        echo "error: the TUI app has not been initialized (missing apps/tui/Cargo.toml)" >&2
        exit 1
    fi

    require_command cargo
}

build_desktop() {
    require_desktop
    echo "==> Building desktop app"
    (cd "$DESKTOP_DIR" && npm run tauri -- build)
}

build_tui() {
    require_tui
    echo "==> Building TUI app"
    cargo build --manifest-path "$TUI_MANIFEST"
}

run_desktop() {
    require_desktop
    echo "==> Running desktop app"
    (cd "$DESKTOP_DIR" && npm run tauri -- dev)
}

run_tui() {
    require_tui
    echo "==> Running TUI app"
    cargo run --manifest-path "$TUI_MANIFEST"
}

test_desktop() {
    require_desktop
    echo "==> Testing desktop frontend"
    (cd "$DESKTOP_DIR" && npm test -- --watch=false)
    echo "==> Testing desktop Rust backend"
    cargo test --manifest-path "$DESKTOP_DIR/src-tauri/Cargo.toml"
    test_desktop_e2e
}

test_desktop_e2e() {
    require_desktop_frontend
    echo "==> Testing desktop UI end to end"
    (cd "$DESKTOP_DIR" && npm run e2e)
}

test_tui() {
    require_tui
    echo "==> Testing TUI app"
    cargo test --manifest-path "$TUI_MANIFEST"
}

case "${1:-}" in
    build-all)
        require_desktop
        require_tui
        build_desktop
        build_tui
        ;;
    build-desktop) build_desktop ;;
    build-tui) build_tui ;;
    run-desktop) run_desktop ;;
    run-tui) run_tui ;;
    test-all)
        require_desktop
        require_tui
        test_desktop
        test_tui
        ;;
    test-desktop) test_desktop ;;
    test-desktop-e2e) test_desktop_e2e ;;
    test-tui) test_tui ;;
    *)
        echo "usage: $0 {build-all|build-desktop|build-tui|run-desktop|run-tui|test-all|test-desktop|test-desktop-e2e|test-tui}" >&2
        exit 2
        ;;
esac
