use crate::agent::AgentRuntime;
use crate::storage::managed_roots::ManagedRootStore;
use crate::document_extraction::extract_document;
use file_engine_cli::journal::read_operation_history;
use file_engine_cli::rules::load_rule_set_for_root;
use serde_json::{json, Value};

pub async fn process_pending_agent_tools(agent: &AgentRuntime, roots: &ManagedRootStore) -> Result<usize, String> {
    let requests = agent.pending_agent_tools().await.map_err(|e| e.to_string())?;
    let mut completed = 0;
    for request in requests {
        if request.step.tool_name == "DOCUMENT_EXTRACT" {
            let root_id = request.step.input.get("rootId").and_then(Value::as_str).unwrap_or_default();
            let path = request.step.input.get("relativePath").and_then(Value::as_str).unwrap_or_default();
            let expected = request.step.input.get("expectedSha256").and_then(Value::as_str).map(ToOwned::to_owned);
            let consent = request.room.get("aiDocumentAnalysisConsent").and_then(Value::as_bool).unwrap_or(false);
            let extraction = roots.get(root_id).and_then(|root| extract_document(&root.root, path, consent, expected.as_deref()));
            match extraction {
                Ok(result) => agent.complete_agent_tool(request.step.id, "SUCCEEDED", serde_json::to_value(result).map_err(|e| e.to_string())?, None).await.map_err(|e| e.to_string())?,
                Err(error) => { agent.complete_agent_tool(request.step.id, "FAILED", json!({}), Some(error.clone())).await.map_err(|e| e.to_string())?; }
            }
            completed += 1;
            continue;
        }
        if request.step.tool_name == "RULE_LIST" {
            let root_id = request.step.input.get("rootId").and_then(Value::as_str).unwrap_or_default();
            let result = roots.get(root_id).and_then(|root| load_rule_set_for_root(&root.root).map_err(|e| e.to_string()));
            match result {
                Ok(rules) => agent.complete_agent_tool(request.step.id, "SUCCEEDED", json!({ "version": rules.version, "rules": rules.rules }), None).await.map_err(|e| e.to_string())?,
                Err(_error) => agent.complete_agent_tool(request.step.id, "FAILED", json!({}), Some("RULE_LIST_FAILED".into())).await.map_err(|e| e.to_string())?,
            }
            completed += 1;
            continue;
        }
        if request.step.tool_name != "OPERATION_HISTORY" {
            let _ = agent.complete_agent_tool(request.step.id, "FAILED", json!({}), Some("UNSUPPORTED_TOOL".into())).await;
            continue;
        }
        let root_id = request.step.input.get("rootId").and_then(Value::as_str).unwrap_or_default();
        let result = roots.get(root_id).and_then(|root| read_operation_history(&root.root).map_err(|e| e.to_string()));
        match result {
            Ok(report) => {
                let operations = report.operations.into_iter().take(50).map(|op| json!({
                    "operationId": op.operation_id,
                    "action": format!("{:?}", op.action),
                    "createdAt": op.created_unix_ms,
                    "canUndo": op.can_undo,
                    "undoBlockedReason": op.undo_blocked_reason,
                })).collect::<Vec<_>>();
                let metadata = json!({ "operations": operations, "corruption": report.corruption.map(|c| json!({ "line": c.line, "message": c.message })) });
                agent.complete_agent_tool(request.step.id, "SUCCEEDED", metadata, None).await.map_err(|e| e.to_string())?;
            }
            Err(error) => {
                agent.complete_agent_tool(request.step.id, "FAILED", json!({}), Some("OPERATION_HISTORY_FAILED".into())).await.map_err(|e| e.to_string())?;
                eprintln!("operation history tool failed: {error}");
            }
        }
        completed += 1;
    }
    Ok(completed)
}
