mod common;

use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::Arc;

struct ProfileInspectingRunner;

impl SubagentRunner for ProfileInspectingRunner {
    fn invoke(
        &self,
        request: InvokeSubagentRequest,
        profile: SubagentProfile,
        inherited_context: Vec<ConversationMessage>,
        stop_token: StopToken,
        remaining_depth: u8,
    ) -> BoxFuture<'static, Result<SubagentOutcome, LlmError>> {
        Box::pin(async move {
            assert_eq!(request.task, "inspect");
            assert_eq!(profile.provider.as_str(), "gemini");
            assert_eq!(profile.policy.permissions(), AllowedPermissions::NONE);
            assert!(inherited_context.is_empty());
            assert!(!stop_token.is_stopped());
            assert_eq!(remaining_depth, 0);
            Ok(SubagentOutcome {
                text: "child result".into(),
                usage: Usage {
                    total_tokens: Some(5),
                    ..Default::default()
                },
            })
        })
    }
}

fn reviewer_profile() -> SubagentProfile {
    SubagentProfile {
        provider: ProviderId::new("gemini").unwrap(),
        model: Some(ModelId::new("gemini-test").unwrap()),
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
async fn named_subagent_profiles_are_fixed_and_usage_is_aggregated() {
    let profile_id = SubagentProfileId::new("reviewer").unwrap();
    let mut profile_registry = SubagentProfileRegistry::default();
    profile_registry
        .register(profile_id.clone(), reviewer_profile())
        .unwrap();
    let provider = ScriptedProvider::new(
        BuiltInProvider::ChatGpt,
        ProviderBehavior::InvokeSubagent(InvokeSubagentRequest {
            task: "inspect".into(),
            profile: profile_id,
        }),
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "gpt-test")
        .subagent_profiles(profile_registry)
        .subagent_runner(Arc::new(ProfileInspectingRunner))
        .build()
        .unwrap();

    let LlmRunOutcome::Completed(response) = runtime
        .request("chatgpt")
        .tools(["invoke_subagent"])
        .allowed(ALL_FILESYSTEM_ACCESS | ALL_USER_COMMANDS)
        .user_message("delegate")
        .send()
        .await
        .unwrap()
    else {
        panic!("expected completion")
    };

    assert_eq!(response.usage.child_usage.len(), 1);
    assert_eq!(response.usage.total_tokens_including_children(), 5);
}
