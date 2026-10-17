use super::*;

#[cfg(unix)]
#[tokio::test]
async fn inherited_secret_environment_is_absent() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let workspace_directory = tempfile::tempdir().unwrap();
    let authorization = prepare_sandboxed_command(
        workspace_directory.path(),
        CommandInvocation {
            executable: OsString::from("sh"),
            arguments: vec![
                OsString::from("-c"),
                OsString::from("printf %s \"${OLLATOM_TEST_SECRET-unset}\""),
            ],
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap();
    let command_entity = execute_sandboxed_command(authorization).unwrap();
    let outcome = command_entity.await.unwrap();

    assert_eq!(outcome.terminal_logs.standard_output, "unset");
}

#[cfg(unix)]
#[tokio::test]
async fn sandbox_cannot_read_a_sibling_directory() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let parent_directory = tempfile::tempdir().unwrap();
    let workspace_directory = parent_directory.path().join("workspace");
    let outside_file = parent_directory.path().join("outside-secret");
    std::fs::create_dir(&workspace_directory).unwrap();
    std::fs::write(&outside_file, "secret").unwrap();
    let command_entity = os::execute_command_in_directory_with_arguments(
        &workspace_directory,
        OsString::from("cat"),
        [outside_file.into_os_string()],
    )
    .unwrap();
    let outcome = command_entity.await.unwrap();

    assert!(!outcome.succeeded);
    assert!(outcome.terminal_logs.standard_output.is_empty());
}

#[test]
fn command_parser_rejects_unclosed_quotes() {
    assert!(matches!(
        execute_command("sh -c 'unterminated"),
        Err(CommandError::InvalidCommand)
    ));
}

#[cfg(windows)]
#[tokio::test]
async fn appcontainer_command_captures_output_and_completes() {
    let workspace_directory = tempfile::tempdir().unwrap();
    let authorization = prepare_windows_test_command(workspace_directory.path(), ["/C", "echo ollatom"]);
    let command_entity = execute_sandboxed_command(authorization).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(20), command_entity)
        .await
        .unwrap()
        .unwrap();

    assert!(outcome.succeeded);
    assert!(outcome.terminal_logs.standard_output.contains("ollatom"));
}

#[cfg(windows)]
#[tokio::test]
async fn appcontainer_job_termination_stops_the_command() {
    let workspace_directory = tempfile::tempdir().unwrap();
    let authorization = prepare_windows_test_command(
        workspace_directory.path(),
        ["/C", "echo started & timeout /T 30 /NOBREAK >NUL"],
    );
    let command_entity = execute_sandboxed_command(authorization).unwrap();
    assert!(
        wait_for_output(&command_entity, Duration::from_secs(10)).await,
        "sandboxed command did not start producing output"
    );

    command_entity.terminate().unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(10), command_entity)
        .await
        .unwrap()
        .unwrap();

    assert!(outcome.terminated);
    assert!(!outcome.succeeded);
}

#[cfg(windows)]
#[tokio::test]
async fn appcontainer_job_stops_descendants_after_the_root_exits() {
    let workspace_directory = tempfile::tempdir().unwrap();
    let script_path = workspace_directory.path().join("spawn-descendant.cmd");
    let descendant_marker = workspace_directory.path().join("descendant.marker");
    std::fs::write(
        &script_path,
        "@echo off\r\nstart \"\" /B cmd.exe /C \"timeout /T 4 /NOBREAK >NUL & echo survived > descendant.marker\"\r\n",
    )
    .unwrap();
    let system_root = std::env::var_os("SystemRoot").unwrap();
    let command_interpreter = PathBuf::from(system_root).join("System32").join("cmd.exe");
    let authorization = prepare_sandboxed_command(
        workspace_directory.path(),
        CommandInvocation {
            executable: command_interpreter.into_os_string(),
            arguments: vec![OsString::from("/C"), script_path.into_os_string()],
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap();
    let command_entity = execute_sandboxed_command(authorization).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(15), command_entity)
        .await
        .unwrap()
        .unwrap();

    assert!(outcome.succeeded);
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!descendant_marker.exists());
}

#[cfg(windows)]
fn prepare_windows_test_command<const ARGUMENT_COUNT: usize>(
    workspace_directory: &std::path::Path,
    arguments: [&str; ARGUMENT_COUNT],
) -> os::SandboxedCommandAuthorization {
    let system_root = std::env::var_os("SystemRoot").unwrap();
    let command_interpreter = PathBuf::from(system_root).join("System32").join("cmd.exe");
    prepare_sandboxed_command(
        workspace_directory,
        CommandInvocation {
            executable: command_interpreter.into_os_string(),
            arguments: arguments.into_iter().map(OsString::from).collect(),
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap()
}

#[cfg(windows)]
async fn wait_for_output(command_entity: &os::CommandEntity, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async {
        loop {
            if !command_entity.terminal_logs().standard_output.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}
