@echo off
setlocal

set "REPOSITORY_ROOT_DIRECTORY=%~dp0.."
set "CRATES_DIRECTORY=%REPOSITORY_ROOT_DIRECTORY%\crates"
set "DESKTOP_DIRECTORY=%REPOSITORY_ROOT_DIRECTORY%\apps\desktop"
set "TUI_MANIFEST_PATH=%REPOSITORY_ROOT_DIRECTORY%\apps\tui\Cargo.toml"

if "%OLLATOM_MISE_EXECUTION_ACTIVE%"=="1" goto dispatch
where mise >nul 2>&1 || (
    echo error: 'mise' is required; install it and run 'mise install' in %REPOSITORY_ROOT_DIRECTORY% 1>&2
    exit /b 1
)
set "OLLATOM_MISE_EXECUTION_ACTIVE=1"
mise exec -C "%REPOSITORY_ROOT_DIRECTORY%" -- "%ComSpec%" /d /s /c ""%~f0" %*"
set "COMMAND_EXIT_CODE=%errorlevel%"
exit /b %COMMAND_EXIT_CODE%

:dispatch
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

:require_desktop_frontend
where npm >nul 2>&1 || (
    echo error: 'npm' is required but was not found on PATH 1>&2
    exit /b 1
)
if not exist "%DESKTOP_DIRECTORY%\node_modules\" (
    echo error: desktop dependencies are missing; run 'npm ci' in apps/desktop 1>&2
    exit /b 1
)
exit /b 0

:require_desktop
call :require_desktop_frontend
if errorlevel 1 exit /b %errorlevel%
where cargo >nul 2>&1 || (
    echo error: 'cargo' is required but was not found on PATH 1>&2
    exit /b 1
)
exit /b 0

:require_tui
if not exist "%TUI_MANIFEST_PATH%" (
    echo error: the TUI app has not been initialized ^(missing apps/tui/Cargo.toml^) 1>&2
    exit /b 1
)
where cargo >nul 2>&1 || (
    echo error: 'cargo' is required but was not found on PATH 1>&2
    exit /b 1
)
exit /b 0

:build_all
call :build_desktop
if errorlevel 1 exit /b %errorlevel%
call :build_tui
exit /b %errorlevel%

:build_desktop
call :require_desktop
if errorlevel 1 exit /b %errorlevel%
echo ==^> Building desktop app
pushd "%DESKTOP_DIRECTORY%"
call npm run tauri -- build
set "COMMAND_EXIT_CODE=%errorlevel%"
popd
exit /b %COMMAND_EXIT_CODE%

:build_tui
call :require_tui
if errorlevel 1 exit /b %errorlevel%
echo ==^> Building TUI app
cargo build --manifest-path "%TUI_MANIFEST_PATH%"
exit /b %errorlevel%

:run_desktop
call :require_desktop
if errorlevel 1 exit /b %errorlevel%
echo ==^> Running desktop app
pushd "%DESKTOP_DIRECTORY%"
call npm run tauri -- dev
set "COMMAND_EXIT_CODE=%errorlevel%"
popd
exit /b %COMMAND_EXIT_CODE%

:run_tui
call :require_tui
if errorlevel 1 exit /b %errorlevel%
echo ==^> Running TUI app
cargo run --manifest-path "%TUI_MANIFEST_PATH%"
exit /b %errorlevel%

:test_all
call :test_crates
if errorlevel 1 exit /b %errorlevel%
call :test_desktop
if errorlevel 1 exit /b %errorlevel%
call :test_tui
exit /b %errorlevel%

:test_desktop
call :require_desktop
if errorlevel 1 exit /b %errorlevel%
echo ==^> Testing desktop frontend
pushd "%DESKTOP_DIRECTORY%"
call npm test -- --watch=false
set "COMMAND_EXIT_CODE=%errorlevel%"
popd
if not "%COMMAND_EXIT_CODE%"=="0" exit /b %COMMAND_EXIT_CODE%
echo ==^> Testing desktop Rust backend
cargo test --manifest-path "%DESKTOP_DIRECTORY%\src-tauri\Cargo.toml"
if errorlevel 1 exit /b %errorlevel%
call :run_desktop_end_to_end_tests
exit /b %errorlevel%

:test_desktop_e2e
call :require_desktop_frontend
if errorlevel 1 exit /b %errorlevel%
call :run_desktop_end_to_end_tests
exit /b %errorlevel%

:run_desktop_end_to_end_tests
echo ==^> Testing desktop UI end to end
setlocal
if defined NO_COLOR (
    set "NO_COLOR="
    set "FORCE_COLOR=0"
)
pushd "%DESKTOP_DIRECTORY%"
call npm run e2e
set "COMMAND_EXIT_CODE=%errorlevel%"
popd
endlocal & exit /b %COMMAND_EXIT_CODE%

:test_crates
where cargo >nul 2>&1 || (
    echo error: 'cargo' is required but was not found on PATH 1>&2
    exit /b 1
)
set "CRATE_MANIFEST_FOUND="
for /r "%CRATES_DIRECTORY%" %%C in (Cargo.toml) do (
    set "CRATE_MANIFEST_FOUND=1"
    for %%D in ("%%~dpC.") do echo ==^> Testing crate %%~nxD
    cargo test --manifest-path "%%~fC"
    if errorlevel 1 exit /b 1
)
if not defined CRATE_MANIFEST_FOUND (
    echo error: no Rust crates were found in %CRATES_DIRECTORY% 1>&2
    exit /b 1
)
exit /b 0

:test_tui
call :require_tui
if errorlevel 1 exit /b %errorlevel%
echo ==^> Testing TUI app
cargo test --manifest-path "%TUI_MANIFEST_PATH%"
exit /b %errorlevel%
