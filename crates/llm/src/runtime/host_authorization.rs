use super::host::RunHost;
use crate::{
    ApprovalDecision, ApprovalRequest, AuthorizationGrant, InteractionReply, InteractionRequest,
    LlmError, RunEvent, ToolEvent, ToolPlan,
};
use std::future::Future;

impl RunHost {
    pub(super) async fn authorize(
        &self,
        tool_plan: &ToolPlan,
        execution_stop_token: crate::StopToken,
    ) -> Result<Option<AuthorizationGrant>, LlmError> {
        let missing_capabilities = tool_plan
            .required_capabilities
            .iter()
            .filter(|capability| !self.policy.permits(capability))
            .cloned()
            .collect::<Vec<_>>();

        if missing_capabilities.is_empty() {
            return Ok(Some(AuthorizationGrant::issue(
                tool_plan,
                execution_stop_token,
            )));
        }

        let approval_request = ApprovalRequest {
            run_id: self.run_id,
            tool_name: tool_plan.call.name.clone(),
            call_id: tool_plan.call.id.clone(),
            capabilities: missing_capabilities,
            summary: format!("{} requests additional capabilities", tool_plan.call.name),
        };
        let can_request_approval = self.authorizer.is_some()
            || self.policy.approval().is_some()
            || self.interaction_callback.is_some()
            || self.policy.streams_approvals();

        if can_request_approval {
            self.events
                .emit(RunEvent::Tool(ToolEvent::ApprovalRequested {
                    call_id: tool_plan.call.id.clone(),
                }))
                .await?;
        }

        let approval_decision = self.request_approval_decision(approval_request).await?;

        Ok((approval_decision == ApprovalDecision::AllowOnce)
            .then(|| AuthorizationGrant::issue(tool_plan, execution_stop_token)))
    }

    async fn request_approval_decision(
        &self,
        approval_request: ApprovalRequest,
    ) -> Result<ApprovalDecision, LlmError> {
        if let Some(authorizer) = &self.authorizer {
            return await_unless_stopped(
                &self.stop,
                authorizer.authorize(approval_request.clone()),
            )
            .await?;
        }

        if let Some(approval_handler) = self.policy.approval() {
            return await_unless_stopped(&self.stop, approval_handler(approval_request)).await;
        }

        if let Some(interaction_callback) = &self.interaction_callback {
            let interaction_request = InteractionRequest::Approval {
                id: crate::InteractionId(0),
                request: approval_request,
            };
            let interaction_reply =
                await_unless_stopped(&self.stop, interaction_callback(interaction_request)).await?;
            return approval_decision_from(interaction_reply);
        }

        if self.policy.denies_untrusted() || !self.policy.streams_approvals() {
            return Ok(ApprovalDecision::Deny);
        }

        let interaction_reply = self
            .interactions
            .request_interaction(
                |interaction_id| InteractionRequest::Approval {
                    id: interaction_id,
                    request: approval_request,
                },
                &self.events,
                &self.stop,
            )
            .await?;

        approval_decision_from(interaction_reply)
    }
}

async fn await_unless_stopped<Output, Operation>(
    stop_token: &crate::StopToken,
    operation: Operation,
) -> Result<Output, LlmError>
where
    Operation: Future<Output = Output>,
{
    tokio::select! {
        _ = stop_token.cancelled() => Err(LlmError::Cancelled),
        output = operation => Ok(output),
    }
}

fn approval_decision_from(
    interaction_reply: InteractionReply,
) -> Result<ApprovalDecision, LlmError> {
    match interaction_reply {
        InteractionReply::Approval(approval_decision) => Ok(approval_decision),
        InteractionReply::UserAnswers(_) => Err(LlmError::ProviderProtocol(
            "approval request received user answers".into(),
        )),
    }
}
