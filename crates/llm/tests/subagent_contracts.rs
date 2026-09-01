mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

struct WaitingRunner {
    child_stop_token: Arc<Mutex<Option<StopToken>>>,
    child_started: Arc<Notify>,
}

struct ContextRunner;

impl SubagentRunner for ContextRunner {
    fn invoke(
        &self,
        _request: InvokeSubagentRequest,
        _profile: SubagentProfile,
        inherited_context: Vec<ConversationMessage>,
        _stop: StopToken,
        remaining_depth: u8,
    ) -> BoxFuture<'static, Result<SubagentOutcome, LlmError>> {
        Box::pin(async move {
            assert_eq!(inherited_context.len(), 1);
            assert_eq!(remaining_depth, 0);
            Ok(SubagentOutcome {
                text: "context received".into(),
                usage: Usage::default(),
            })
        })
    }
}

impl SubagentRunner for WaitingRunner {
    fn invoke(
        &self,
        _request: InvokeSubagentRequest,
        _profile: SubagentProfile,
        _inherited_context: Vec<ConversationMessage>,
        stop_token: StopToken,
        _remaining_depth: u8,
    ) -> BoxFuture<'static, Result<SubagentOutcome, LlmError>> {
        *self.child_stop_token.lock().unwrap() = Some(stop_token);
        self.child_started.notify_waiters();
        Box::pin(std::future::pending())
    }
}

fn default_subagent_profile() -> SubagentProfile {
    SubagentProfile {
        provider: ProviderId::new("gemini").unwrap(),
        model: Some(ModelId::new("child-model").unwrap()),
        effort: ReasoningEffort::Low,
        system_prompt: Some("review".into()),
        tool_names: vec![],
        policy: RunPolicy::default(),
        max_tokens: Some(100),
        timeout: None,
        allow_context: false,
        max_depth: 1,
        options: LlmOptions::default(),
    }
}

#[tokio::test]
async fn unknown_profiles_are_model_visible_tool_failures() {
    let tool_call = ToolCall {
        id: "child-1".into(),
        name: "invoke_subagent".into(),
        arguments: serde_json::json!({"task":"inspect","profile":"missing"}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::ChatGpt,
        ProviderBehavior::ExecuteToolRound(vec![tool_call]),
    );
    let child_stop_token = Arc::new(Mutex::new(None));
    let mut profiles = SubagentProfileRegistry::default();
    profiles
        .register(
            SubagentProfileId::new("known").unwrap(),
            default_subagent_profile(),
        )
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .subagent_profiles(profiles)
        .subagent_runner(Arc::new(WaitingRunner {
            child_stop_token,
            child_started: Arc::new(Notify::new()),
        }))
        .build()
        .unwrap();
    let LlmRunOutcome::Completed(response) = runtime
        .request("chatgpt")
        .tools(["invoke_subagent"])
        .user_message("delegate")
        .send()
        .await
        .unwrap()
    else {
        panic!("expected provider recovery");
    };
    assert!(matches!(
        response.tool_executions[0].output.error,
        Some(ToolFailure::InvalidArguments(_))
    ));
}

#[tokio::test]
async fn parent_cancellation_stops_an_active_child() {
    let profile_id = SubagentProfileId::new("known").unwrap();
    let tool_call = ToolCall {
        id: "child-1".into(),
        name: "invoke_subagent".into(),
        arguments: serde_json::json!({"task":"inspect","profile":"known"}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Claude,
        ProviderBehavior::ExecuteToolRound(vec![tool_call]),
    );
    let child_stop_token = Arc::new(Mutex::new(None));
    let child_started = Arc::new(Notify::new());
    let mut profiles = SubagentProfileRegistry::default();
    profiles
        .register(profile_id, default_subagent_profile())
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .subagent_profiles(profiles)
        .subagent_runner(Arc::new(WaitingRunner {
            child_stop_token: child_stop_token.clone(),
            child_started: child_started.clone(),
        }))
        .build()
        .unwrap();
    let run = runtime
        .request("claude")
        .tools(["invoke_subagent"])
        .user_message("delegate")
        .send();
    let stop_handle = run.stop_handle();
    let child_started_notification = child_started.notified();
    let run_task = tokio::spawn(run);

    if child_stop_token.lock().unwrap().is_none() {
        child_started_notification.await;
    }

    stop_handle.stop();
    assert!(matches!(
        run_task.await.unwrap().unwrap(),
        LlmRunOutcome::Cancelled(_)
    ));
    let child_was_stopped = child_stop_token
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .is_stopped();
    assert!(child_was_stopped);
}

#[tokio::test]
async fn profiles_explicitly_control_context_inheritance_and_child_depth() {
    let profile_id = SubagentProfileId::new("contextual").unwrap();
    let mut child_profile = default_subagent_profile();
    child_profile.allow_context = true;
    child_profile.max_depth = 1;
    let mut profiles = SubagentProfileRegistry::default();
    profiles
        .register(profile_id.clone(), child_profile)
        .unwrap();
    let provider = ScriptedProvider::new(
        BuiltInProvider::LmStudio,
        ProviderBehavior::InvokeSubagent(InvokeSubagentRequest {
            task: "inspect".into(),
            profile: profile_id,
        }),
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .subagent_profiles(profiles)
        .subagent_runner(Arc::new(ContextRunner))
        .build()
        .unwrap();
    let context = [ConversationMessage {
        role: ConversationRole::User,
        content: vec![ContentBlock::text("earlier")],
    }];
    let limits = RunLimits {
        subagent_depth: 4,
        ..Default::default()
    };
    assert!(matches!(
        runtime
            .request("lm-studio")
            .tools(["invoke_subagent"])
            .context(context)
            .limits(limits)
            .user_message("delegate")
            .send()
            .await
            .unwrap(),
        LlmRunOutcome::Completed(_)
    ));
}
