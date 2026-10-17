use super::*;

#[tokio::test]
async fn command_entity_captures_standard_output_and_errors() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let command_entity = execute_command("sh -c 'printf output; printf error >&2'").unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.succeeded);
    assert_eq!(outcome.terminal_logs().standard_output, "output");
    assert_eq!(outcome.terminal_logs().standard_error, "error");
    assert!(outcome.terminal_logs().combined_output.contains("output"));
    assert!(outcome.terminal_logs().combined_output.contains("error"));
}

#[cfg(unix)]
#[tokio::test]
async fn command_entity_can_be_terminated() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let command_entity = execute_command("sleep 30").unwrap();
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
    assert!(!outcome.succeeded);
}

#[cfg(unix)]
#[tokio::test]
async fn command_entity_exposes_logs_before_completion() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let command_entity = execute_command("sh -c 'printf start; sleep 1; printf end'").unwrap();

    assert!(
        wait_for_condition(Duration::from_secs(2), || {
            command_entity.terminal_logs().standard_output.contains("start")
        })
        .await
    );
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
}

#[cfg(unix)]
#[tokio::test]
async fn termination_is_not_reported_after_natural_completion() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let mut command_entity = execute_command("true").unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(2), &mut command_entity)
        .await
        .unwrap()
        .unwrap();
    command_entity.terminate().unwrap();
    assert!(outcome.succeeded);
    assert!(!outcome.terminated);
}

#[cfg(unix)]
#[tokio::test]
async fn termination_stops_descendant_processes() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let completion_marker = unique_completion_marker();
    let started_marker = unique_completion_marker();
    let script = format!(
        "(printf started > {}; sleep 1; printf survived > {}) & wait",
        shlex::try_quote(&started_marker.to_string_lossy()).unwrap(),
        shlex::try_quote(&completion_marker.to_string_lossy()).unwrap(),
    );
    let command = shlex::try_join(["sh", "-c", &script]).unwrap();
    let command_entity = execute_command(command).unwrap();
    assert!(file_appears_within(&started_marker, Duration::from_secs(1)).await);
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
    assert!(!file_appears_within(&completion_marker, Duration::from_secs(2)).await);
    std::fs::remove_file(started_marker).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn normal_root_exit_stops_descendants_before_waiting_for_pipe_eof() {
    if !sandbox_backend_can_launch() {
        return;
    }
    let completion_marker = unique_completion_marker();
    let script = format!(
        "(sleep 0.5; printf survived > {}; printf late-output; printf late-error >&2) & printf root-output; exit 0",
        shlex::try_quote(&completion_marker.to_string_lossy()).unwrap()
    );
    let command = shlex::try_join(["sh", "-c", &script]).unwrap();
    let command_entity = execute_command(command).unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(1), command_entity)
        .await
        .expect("command completion should not wait for descendant pipes")
        .unwrap();

    assert!(outcome.succeeded);
    assert_eq!(outcome.terminal_logs.standard_output, "root-output");
    assert!(outcome.terminal_logs.standard_error.is_empty());
    assert!(!file_appears_within(&completion_marker, Duration::from_secs(1)).await);
}

#[cfg(unix)]
#[tokio::test]
async fn command_authorization_rejects_a_replaced_executable() {
    let workspace_directory = tempfile::tempdir().unwrap();
    let executable_path = workspace_directory.path().join("approved-tool");
    let replacement_path = workspace_directory.path().join("replacement-tool");
    std::fs::copy("/bin/true", &executable_path).unwrap();
    let authorization = prepare_sandboxed_command(
        workspace_directory.path(),
        CommandInvocation {
            executable: executable_path.clone().into_os_string(),
            arguments: Vec::new(),
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap();

    std::fs::copy("/bin/false", &replacement_path).unwrap();
    std::fs::rename(replacement_path, &executable_path).unwrap();

    assert!(matches!(
        execute_sandboxed_command(authorization),
        Err(CommandError::InvalidAuthorization(_))
    ));
}

#[cfg(unix)]
#[test]
fn bare_executable_authorization_freezes_canonical_path_and_identity() {
    use std::os::unix::fs::MetadataExt;

    let workspace_directory = tempfile::tempdir().unwrap();
    let authorization = prepare_sandboxed_command(
        workspace_directory.path(),
        CommandInvocation {
            executable: OsString::from("sh"),
            arguments: Vec::new(),
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap();
    let canonical_executable_path = std::fs::canonicalize(&authorization.executable_path).unwrap();
    let executable_metadata = std::fs::metadata(&canonical_executable_path).unwrap();

    assert!(authorization.executable_path.is_absolute());
    assert_eq!(authorization.executable_path, canonical_executable_path);
    assert_eq!(
        authorization.executable_identity,
        os::ExecutableIdentity::Unix {
            device_identifier: executable_metadata.dev(),
            inode_identifier: executable_metadata.ino(),
        }
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn pinned_executable_launch_preserves_argv0_and_null_stdin() {
    if !sandbox_backend_can_launch() {
        return;
    }
    use std::os::unix::fs::PermissionsExt;

    let workspace_directory = tempfile::tempdir().unwrap();
    let executable_path = workspace_directory.path().join("approved-script");
    std::fs::write(
        &executable_path,
        "#!/bin/sh\ninput=$(cat)\nprintf '%s|%s|%s' \"$0\" \"$1\" \"$input\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&executable_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let authorization = prepare_sandboxed_command(
        workspace_directory.path(),
        CommandInvocation {
            executable: executable_path.clone().into_os_string(),
            arguments: vec![OsString::from("argument")],
        },
        BTreeMap::new(),
        SandboxNetworkPolicy::Denied,
    )
    .unwrap();

    let outcome = execute_sandboxed_command(authorization).unwrap().await.unwrap();

    assert!(outcome.succeeded);
    assert_eq!(
        outcome.terminal_logs.standard_output,
        format!("{}|argument|", executable_path.display())
    );
}
