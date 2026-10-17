use os::{CommandError, execute_command};
#[cfg(any(unix, windows))]
use os::{CommandInvocation, SandboxNetworkPolicy, execute_sandboxed_command, prepare_sandboxed_command};
#[cfg(unix)]
use std::{
    collections::BTreeMap, ffi::OsString, path::PathBuf, process::Command as ProcessCommand, sync::OnceLock,
    time::Duration,
};
#[cfg(windows)]
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

#[cfg(unix)]
#[cfg(unix)]
async fn file_appears_within(file_path: &std::path::Path, timeout: Duration) -> bool {
    wait_for_condition(timeout, || file_path.exists()).await
}

#[cfg(unix)]
async fn wait_for_condition<Condition>(timeout: Duration, mut condition: Condition) -> bool
where
    Condition: FnMut() -> bool,
{
    tokio::time::timeout(timeout, async {
        loop {
            if condition() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}

#[cfg(unix)]
fn unique_completion_marker() -> PathBuf {
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("ollatom-os-descendant-{}-{unique_suffix}", std::process::id()))
}

#[cfg(unix)]
fn sandbox_backend_can_launch() -> bool {
    static BACKEND_CAN_LAUNCH: OnceLock<bool> = OnceLock::new();
    *BACKEND_CAN_LAUNCH.get_or_init(|| {
        #[cfg(target_os = "linux")]
        let probe_result = ProcessCommand::new("/usr/bin/bwrap")
            .args(["--unshare-all", "--proc", "/proc", "--dev", "/dev", "/usr/bin/true"])
            .output();

        #[cfg(target_os = "macos")]
        let probe_result = ProcessCommand::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1) (allow default)", "/usr/bin/true"])
            .output();

        probe_result.is_ok_and(|output| output.status.success())
    })
}

#[path = "command/process_lifecycle.rs"]
mod process_lifecycle;
#[path = "command/sandbox_and_platform.rs"]
mod sandbox_and_platform;
