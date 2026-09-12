use super::tool_support;
use crate::{
    AuthorizationGrant, BoxFuture, RequiredCapability, Tool, ToolCall, ToolDefinition, ToolFailure,
    ToolOutput, ToolPlan,
};
use os::{search_target_host, web_search};
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct WebSearchTool {
    definition: ToolDefinition,
}

impl WebSearchTool {
    pub(super) fn new() -> Self {
        Self {
            definition: tool_support::create_tool_definition(
                "web_search",
                "Fetch a web page or search Google and return filtered text.",
                json!({"query": {"type": "string", "minLength": 1}}),
                &["query"],
            ),
        }
    }
}

impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn plan(&self, tool_call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        tool_support::validate_tool_call(&self.definition, tool_call)?;
        let search_query = parse_search_query(&tool_call.arguments)?;
        let network_host = search_target_host(search_query)
            .map_err(|error| ToolFailure::InvalidArguments(error.to_string()))?;

        Ok(tool_support::create_tool_plan(
            tool_call,
            tool_call.arguments.clone(),
            vec![RequiredCapability::Network { host: network_host }],
            None,
        ))
    }

    fn execute(
        &self,
        tool_plan: ToolPlan,
        _authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        Box::pin(async move {
            let search_query = parse_search_query(&tool_plan.normalized_arguments)?;
            let web_search_response = web_search(search_query.to_owned()).await;
            let filtered_text = web_search_response
                .filter_html_code()
                .map_err(|error| ToolFailure::Execution(error.to_string()))?;

            Ok(ToolOutput::success(
                tool_plan.call.id,
                json!({
                    "tool": "web_search",
                    "input": web_search_response.input,
                    "target_url": web_search_response.target_url,
                    "status_code": web_search_response.status_code,
                    "text": filtered_text,
                }),
            ))
        })
    }
}

fn parse_search_query(tool_arguments: &Value) -> Result<&str, ToolFailure> {
    tool_arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolFailure::InvalidArguments("query must be a string".into()))
}
