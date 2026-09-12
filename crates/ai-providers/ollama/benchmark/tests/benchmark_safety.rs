use llm::{
    BasicToolConfiguration, ContentBlock, ConversationRole, RunEvent, RunId, SequencedEvent,
    ToolCall, ToolEvent, ToolOutput, register_basic_tools,
};
use ollama_benchmark::{
    BenchmarkArtifacts, ConversationHistoryRecorder, ModelToolActivity, StageLogger,
};
use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_directory() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ollama-benchmark-test-{suffix}"));
    fs::create_dir_all(&path).expect("temporary directory should be created");
    path
}

fn prepared_artifacts() -> (Arc<BenchmarkArtifacts>, PathBuf) {
    let directory = temporary_directory();
    let artifacts = Arc::new(BenchmarkArtifacts::from_directory(directory.clone()));
    artifacts.initialize().expect("logs should initialize");
    artifacts
        .prepare_project_for_generation()
        .expect("project should be prepared");
    (artifacts, directory)
}

#[test]
fn ownership_marker_controls_replacement() {
    let (artifacts, directory) = prepared_artifacts();
    fs::write(artifacts.project_root().join("old.txt"), "old").expect("file should write");
    artifacts
        .prepare_project_for_generation()
        .expect("owned project should replace");
    assert!(!artifacts.project_root().join("old.txt").exists());
    fs::remove_file(artifacts.ownership_marker()).expect("marker should remove");
    assert!(artifacts.prepare_project_for_generation().is_err());
    fs::remove_dir_all(directory).expect("temporary directory should remove");
}

#[test]
fn benchmark_registers_exact_tool_set() {
    let (artifacts, directory) = prepared_artifacts();
    let registry = register_basic_tools(
        BasicToolConfiguration::new(artifacts.project_root())
            .exclude_directory("node_modules")
            .exclude_directory("dist"),
    )
    .expect("tools should register");
    let names = registry
        .definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "create_directory",
            "delete_path",
            "execute_command",
            "list_files",
            "read_file",
            "rename_path",
            "web_search",
            "write_file",
        ]
    );
    fs::remove_dir_all(directory).expect("temporary directory should remove");
}

#[test]
fn every_tool_event_is_logged_with_available_tool_details() {
    let directory = temporary_directory();
    let log_path = directory.join("events.log");
    let logger = StageLogger::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .expect("event log should open"),
    );
    let tool_call = ToolCall {
        id: "call-1".into(),
        name: "execute_command".into(),
        arguments: serde_json::json!({"command": "npm run build"}),
    };
    let events = [
        RunEvent::Tool(ToolEvent::Planned {
            call: tool_call.clone(),
        }),
        RunEvent::Tool(ToolEvent::ApprovalRequested {
            call_id: tool_call.id.clone(),
        }),
        RunEvent::Tool(ToolEvent::Started {
            call: tool_call.clone(),
        }),
        RunEvent::Tool(ToolEvent::Finished {
            output: ToolOutput::success(
                tool_call.id.clone(),
                serde_json::json!({
                    "tool": "execute_command",
                    "result": {"command": "npm run build", "succeeded": true}
                }),
            ),
        }),
    ];

    for (sequence, event) in events.into_iter().enumerate() {
        logger
            .write_event(&SequencedEvent {
                run_id: RunId(7),
                sequence: u64::try_from(sequence).expect("sequence should fit") + 1,
                event,
            })
            .expect("event should log");
    }
    drop(logger);
    let log_content = fs::read_to_string(&log_path).expect("event log should read");
    assert!(log_content.contains("tool_planned"));
    assert!(log_content.contains("tool_approval_requested"));
    assert!(log_content.contains("tool_started"));
    assert!(log_content.contains("tool_finished"));
    assert!(log_content.contains("execute_command"));
    assert!(log_content.contains("call-1"));
    assert!(log_content.contains("npm run build"));
    let _ = File::open(&log_path).expect("event log should remain readable");
    fs::remove_dir_all(directory).expect("temporary directory should remove");
}

#[test]
fn conversation_history_preserves_parallel_tool_results_as_separate_messages() {
    let first_tool_call = ToolCall {
        id: "call-1".into(),
        name: "read_file".into(),
        arguments: serde_json::json!({"path": "src/App.tsx"}),
    };
    let second_tool_call = ToolCall {
        id: "call-2".into(),
        name: "read_file".into(),
        arguments: serde_json::json!({"path": "src/main.tsx"}),
    };
    let mut history_recorder = ConversationHistoryRecorder::default();
    history_recorder.record(&RunEvent::ProviderRoundStarted { round: 1 });
    history_recorder.record(&RunEvent::ReasoningSummaryDelta("inspect files".into()));
    history_recorder.record(&RunEvent::Tool(ToolEvent::Planned {
        call: first_tool_call.clone(),
    }));
    history_recorder.record(&RunEvent::Tool(ToolEvent::Planned {
        call: second_tool_call.clone(),
    }));
    history_recorder.record(&RunEvent::Tool(ToolEvent::Finished {
        output: ToolOutput::success(first_tool_call.id, serde_json::json!({"content": "one"})),
    }));
    history_recorder.record(&RunEvent::Tool(ToolEvent::Finished {
        output: ToolOutput::success(second_tool_call.id, serde_json::json!({"content": "two"})),
    }));

    let conversation_history = history_recorder.into_conversation_history("continue");

    assert_eq!(conversation_history.len(), 4);
    assert_eq!(conversation_history[0].role, ConversationRole::User);
    assert_eq!(conversation_history[1].role, ConversationRole::Assistant);
    assert!(matches!(
        conversation_history[1].content.first(),
        Some(ContentBlock::ReasoningSummary { .. })
    ));

    for tool_message in &conversation_history[2..] {
        assert_eq!(tool_message.role, ConversationRole::Tool);
        assert_eq!(tool_message.content.len(), 1);
        assert!(matches!(
            tool_message.content.first(),
            Some(ContentBlock::ToolResult { .. })
        ));
    }
}

#[test]
fn agent_stage_requires_a_model_invocation_and_successful_build() {
    let build_tool_call = ToolCall {
        id: "build-call".into(),
        name: "execute_command".into(),
        arguments: serde_json::json!({"command": "npm run build"}),
    };
    let mut model_tool_activity = ModelToolActivity::default();

    assert!(
        model_tool_activity
            .require_successful_agent_stage(3)
            .is_err()
    );
    model_tool_activity.record(&RunEvent::Tool(ToolEvent::Started {
        call: build_tool_call.clone(),
    }));
    assert!(
        model_tool_activity
            .require_successful_agent_stage(3)
            .is_err()
    );
    model_tool_activity.record(&RunEvent::Tool(ToolEvent::Finished {
        output: ToolOutput::success(
            build_tool_call.id,
            serde_json::json!({
                "tool": "execute_command",
                "result": {"command": "npm run build", "succeeded": true}
            }),
        ),
    }));

    assert_eq!(model_tool_activity.invocation_count(), 1);
    assert_eq!(model_tool_activity.successful_build_count(), 1);
    assert!(
        model_tool_activity
            .require_successful_agent_stage(3)
            .is_ok()
    );
}
