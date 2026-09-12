use os::{CommandError, execute_command};
use std::{path::PathBuf, time::Duration};

#[tokio::test]
async fn command_entity_captures_standard_output_and_errors() {
    let command_entity = execute_command("sh -c 'printf output; printf error >&2'").unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.succeeded);
    assert_eq!(outcome.terminal_logs(), outcome.get_terminal_logs());
    assert_eq!(outcome.get_terminal_logs().standard_output, "output");
    assert_eq!(outcome.get_terminal_logs().standard_error, "error");
}

#[tokio::test]
async fn command_entity_can_be_terminated() {
    let command_entity = execute_command("sleep 30").unwrap();
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
    assert!(!outcome.succeeded);
}

#[tokio::test]
async fn command_entity_exposes_logs_before_completion() {
    let command_entity = execute_command("sh -c 'printf start; sleep 1; printf end'").unwrap();

    for _ in 0..20 {
        if command_entity
            .get_terminal_logs()
            .standard_output
            .contains("start")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        command_entity
            .get_terminal_logs()
            .standard_output
            .contains("start")
    );
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
}

#[tokio::test]
async fn termination_is_not_reported_after_natural_completion() {
    let command_entity = execute_command("true").unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.succeeded);
    assert!(!outcome.terminated);
}

#[cfg(unix)]
#[tokio::test]
async fn termination_stops_descendant_processes() {
    let completion_marker = unique_completion_marker();
    let script = format!(
        "(sleep 2; printf survived > {}) & wait",
        shlex::try_quote(&completion_marker.to_string_lossy()).unwrap()
    );
    let command = shlex::try_join(["sh", "-c", &script]).unwrap();
    let command_entity = execute_command(command).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    command_entity.terminate().unwrap();
    let outcome = command_entity.await.unwrap();
    assert!(outcome.terminated);
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!completion_marker.exists());
}

#[test]
fn command_parser_rejects_unclosed_quotes() {
    assert!(matches!(
        execute_command("sh -c 'unterminated"),
        Err(CommandError::InvalidCommand)
    ));
}

fn unique_completion_marker() -> PathBuf {
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ollatom-os-descendant-{}-{unique_suffix}",
        std::process::id()
    ))
}
