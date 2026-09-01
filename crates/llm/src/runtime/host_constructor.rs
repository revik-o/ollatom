use super::{
    host::{HostState, RunHost},
    interactions::InteractionHub,
};
use crate::{
    RunId, RunLimits, RunPolicy, StopToken, SubagentProfileRegistry, ToolRegistry,
    events::EventDispatcher, interaction::InteractionCallback, subagent::SharedSubagentRunner,
};
use std::{collections::BTreeSet, sync::Arc};
use tokio::sync::Mutex;

pub(super) struct RunHostDependencies {
    pub run_id: RunId,
    pub event_dispatcher: Arc<EventDispatcher>,
    pub tool_registry: Arc<ToolRegistry>,
    pub selected_tools: BTreeSet<String>,
    pub policy: RunPolicy,
    pub stop_token: StopToken,
    pub limits: RunLimits,
    pub interaction_hub: Arc<InteractionHub>,
    pub interaction_callback: Option<InteractionCallback>,
    pub subagent_profiles: Arc<SubagentProfileRegistry>,
    pub subagent_runner: Option<SharedSubagentRunner>,
    pub tool_authorizer: Option<Arc<dyn crate::ToolAuthorizer>>,
    pub parent_context: Vec<crate::ConversationMessage>,
}

impl RunHost {
    pub fn new(dependencies: RunHostDependencies) -> Self {
        Self {
            run_id: dependencies.run_id,
            events: dependencies.event_dispatcher,
            tools: dependencies.tool_registry,
            selected_tools: dependencies.selected_tools,
            policy: dependencies.policy,
            stop: dependencies.stop_token,
            limits: dependencies.limits,
            interactions: dependencies.interaction_hub,
            interaction_callback: dependencies.interaction_callback,
            profiles: dependencies.subagent_profiles,
            subagent_runner: dependencies.subagent_runner,
            authorizer: dependencies.tool_authorizer,
            parent_context: dependencies.parent_context,
            state: Mutex::new(HostState::default()),
            execution: Mutex::new(()),
        }
    }
}
