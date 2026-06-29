//! Environment template gate (051-phi-policy-env-gates, US1).
//!
//! Checks `mcpTemplate` against the tool's category and returns `ENV_GATE_BLOCKED`
//! when the template forbids it. Gate [1] in `dispatch_gate`.

use crate::iris::workspace_config::{McpTemplate, ToolCategory};

/// Tool categories blocked per template.
const LIVE_BLOCKED: &[ToolCategory] = &[
    ToolCategory::Execute,
    ToolCategory::Compile,
    ToolCategory::SourceControl,
];

const TEST_BLOCKED: &[ToolCategory] = &[ToolCategory::Execute, ToolCategory::Compile];

/// Check whether `tool_name` is blocked by the active `mcpTemplate`.
///
/// Returns `Some(error_json)` when blocked, `None` when permitted.
pub fn check_env_gate(
    tool_name: &str,
    template: &McpTemplate,
    server_name: &str,
) -> Option<serde_json::Value> {
    let blocked = match template {
        McpTemplate::Dev => return None,
        McpTemplate::Live => LIVE_BLOCKED,
        McpTemplate::Test => TEST_BLOCKED,
    };

    let category = crate::iris::server_manager::tool_to_category_pub(tool_name)?;
    if blocked.contains(&category) {
        let template_str = match template {
            McpTemplate::Dev => "dev",
            McpTemplate::Test => "test",
            McpTemplate::Live => "live",
        };
        return Some(serde_json::json!({
            "error_code": "ENV_GATE_BLOCKED",
            "env_gate_blocked": true,
            "server_name": server_name,
            "template": template_str,
            "blocked_category": category.as_str(),
            "message": format!(
                "Tool '{}' is blocked by environment template '{}' for server '{}'. \
                 Category '{}' is not permitted in {} mode.",
                tool_name, template_str, server_name, category.as_str(), template_str
            ),
            "remediation": format!(
                "Set mcpTemplate = \"dev\" or mcpTemplate = \"test\" in [policy.{}] of .iris-agentic-dev.toml",
                server_name
            ),
        }));
    }

    None
}
