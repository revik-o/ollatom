@echo off
setlocal

set "ROOT_DIR=%~dp0.."
set "CRATES_DIR=%ROOT_DIR%\crates"
set "DESKTOP_DIR=%ROOT_DIR%\apps\desktop"
set "TUI_MANIFEST=%ROOT_DIR%\apps\tui\Cargo.toml"

if /I "%~1"=="build-all" goto build_all
if /I "%~1"=="build-desktop" goto build_desktop
if /I "%~1"=="build-tui" goto build_tui
if /I "%~1"=="run-desktop" goto run_desktop
if /I "%~1"=="run-tui" goto run_tui
if /I "%~1"=="test-all" goto test_all
if /I "%~1"=="test-desktop" goto test_desktop
if /I "%~1"=="test-desktop-e2e" goto test_desktop_e2e
if /I "%~1"=="test-crates" goto test_crates
if /I "%~1"=="test-tui" goto test_tui

echo Usage: %~nx0 ^{build-all^|build-desktop^|build-tui^|run-desktop^|run-tui^|test-all^|test-desktop^|test-desktop-e2e^|test-crates^|test-tui^} 1>&2
exit /b 2

:require_frontend
where npm >nul 2>&1 || (echo error: 'npm' is required but was not found on PATH 1>&2 & exit /b 1)
if not exist "%DESKTOP_DIR%\node_modules\" (
    echo error: desktop dependencies are missing; run 'npm ci' in apps/desktop 1>&2
    exit /b 1
)
exit /b 0

:require_npm
call :require_frontend || exit /b 1
where cargo >nul 2>&1 || (echo error: 'cargo' is required but was not found on PATH 1>&2 & exit /b 1)
exit /b 0

:require_tui
if not exist "%TUI_MANIFEST%" (
    echo error: the TUI app has not been initialized ^(missing apps/tui/Cargo.toml^) 1>&2
    exit /b 1
)
where cargo >nul 2>&1 || (echo error: 'cargo' is required but was not found on PATH 1>&2 & exit /b 1)
exit /b 0

:build_all
call :require_npm || exit /b 1
call :require_tui || exit /b 1
call :do_build_desktop || exit /b 1
call :do_build_tui
exit /b %errorlevel%

:build_desktop
call :require_npm || exit /b 1
call :do_build_desktop
exit /b %errorlevel%

:build_tui
call :require_tui || exit /b 1
call :do_build_tui
exit /b %errorlevel%

:run_desktop
call :require_npm || exit /b 1
echo ==^> Running desktop app
pushd "%DESKTOP_DIR%"
call npm run tauri -- dev
set "RESULT=%errorlevel%"
popd
exit /b %RESULT%

:run_tui
call :require_tui || exit /b 1
echo ==^> Running TUI app
cargo run --manifest-path "%TUI_MANIFEST%"
exit /b %errorlevel%

:test_all
call :require_npm || exit /b 1
call :require_tui || exit /b 1
call :do_test_desktop || exit /b 1
call :do_test_tui
exit /b %errorlevel%

:test_desktop
call :require_npm || exit /b 1
call :do_test_desktop
exit /b %errorlevel%

:test_tui
call :require_tui || exit /b 1
call :do_test_tui
exit /b %errorlevel%

:test_desktop_e2e
call :require_frontend || exit /b 1
call :do_test_desktop_e2e
exit /b %errorlevel%

:test_crates
where cargo >nul 2>&1 || (echo error: 'cargo' is required but was not found on PATH 1>&2 & exit /b 1)
call :do_test_crates
exit /b %errorlevel%

:do_build_desktop
echo ==^> Building desktop app
pushd "%DESKTOP_DIR%"
call npm run tauri -- build
set "RESULT=%errorlevel%"
popd
exit /b %RESULT%

:do_build_tui
echo ==^> Building TUI app
cargo build --manifest-path "%TUI_MANIFEST%"
exit /b %errorlevel%

:do_test_desktop
echo ==^> Testing desktop frontend
pushd "%DESKTOP_DIR%"
call npm test -- --watch=false
set "RESULT=%errorlevel%"
popd
if not "%RESULT%"=="0" exit /b %RESULT%
echo ==^> Testing desktop Rust backend
cargo test --manifest-path "%DESKTOP_DIR%\src-tauri\Cargo.toml"
if errorlevel 1 exit /b %errorlevel%
call :do_test_desktop_e2e
exit /b %errorlevel%

:do_test_desktop_e2e
echo ==^> Testing desktop UI end to end
pushd "%DESKTOP_DIR%"
call npm run e2e
set "RESULT=%errorlevel%"
popd
exit /b %RESULT%

:do_test_crates
set "CRATE_MANIFEST_FOUND="
for /d %%D in ("%CRATES_DIR%\*") do (
    if exist "%%~fD\Cargo.toml" (
        set "CRATE_MANIFEST_FOUND=1"
        echo ==^> Testing crate %%~nxD
        cargo test --manifest-path "%%~fD\Cargo.toml"
        if errorlevel 1 exit /b 1
    )
)
if not defined CRATE_MANIFEST_FOUND (
    echo error: no Rust crates were found in %CRATES_DIR% 1>&2
    exit /b 1
)
exit /b 0

:do_test_tui
echo ==^> Testing TUI app
cargo test --manifest-path "%TUI_MANIFEST%"
exit /b %errorlevel%
