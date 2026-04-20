//! iris_source_control — SCM status, menu, checkout, execute via Atelier xecute.

use schemars::JsonSchema;
use serde::Deserialize;
use crate::iris::connection::IrisConnection;
use crate::elicitation::{ElicitationStore, ElicitationAction};

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn default_namespace() -> String { "USER".to_string() }

/// Known menu item names to probe via OnMenuItem.
pub const KNOWN_MENU_ITEMS: &[&str] = &[
    "CheckOut",
    "UndoCheckOut",
    "CheckIn",
    "GetLatest",
    "Status",
    "History",
    "AddToSourceControl",
];

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScmParams {
    /// Action: status, menu, checkout, execute
    pub action: String,
    pub document: Option<String>,
    /// SCM action ID for action=execute
    pub action_id: Option<String>,
    /// Elicitation resume answer
    pub answer: Option<String>,
    pub elicitation_id: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

async fn xecute(
    iris: &IrisConnection,
    client: &reqwest::Client,
    code: &str,
    namespace: &str,
) -> anyhow::Result<String> {
    let url = iris.atelier_url(&format!("/v1/{}/action/xecute", namespace));
    let resp = client.post(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .json(&serde_json::json!({"expression": code}))
        .send().await?;
    let body: serde_json::Value = resp.json().await?;
    Ok(body["result"]["content"][0]["content"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default())
}

pub async fn handle_iris_source_control(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: ScmParams,
    elicitation_store: &ElicitationStore,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let doc = p.document.as_deref().unwrap_or("");
    let ns = &p.namespace;

    // Handle elicitation resume
    if let (Some(eid), Some(answer)) = (&p.elicitation_id, &p.answer) {
        if let Some(pending) = elicitation_store.lookup(eid) {
            elicitation_store.clear(eid);
            let action_id = pending.scm_action_id.as_deref().unwrap_or("");
            let after_code = format!(
                "set sc=##class(%Studio.SourceControl.Base).AfterUserAction(0,\"{}\",\"{}\",{},\"{}\") write $system.Status.GetErrorText(sc)",
                action_id.replace('"', "\\\""),
                pending.document.replace('"', "\\\""),
                if answer == "yes" { "1" } else { "0" },
                answer.replace('"', "\\\"")
            );
            let out = xecute(iris, client, &after_code, &pending.namespace).await.unwrap_or_default();
            if out.is_empty() || out.starts_with("$") {
                return ok_json(serde_json::json!({"success": true, "document": pending.document, "action_id": action_id}));
            }
            return err_json("SCM_ERROR", &out);
        }
        return err_json("ELICITATION_EXPIRED", "Elicitation session expired or not found");
    }

    match p.action.as_str() {
        "status" => {
            // Check if SCM is installed
            let check_code = format!(
                "set obj=##class(%Studio.SourceControl.Base).%GetImplementationObject(\"{}\") if '$IsObject(obj) {{ write \"UNCONTROLLED\" }} else {{ set editable=obj.IsEditable(\"{}\") write editable_\"|\"_$get(obj.Owner) }}",
                doc.replace('"', "\\\""),
                doc.replace('"', "\\\"")
            );
            let out = xecute(iris, client, &check_code, ns).await.unwrap_or("UNCONTROLLED".to_string());
            if out.trim() == "UNCONTROLLED" || out.is_empty() {
                return ok_json(serde_json::json!({"success":true,"controlled":false,"editable":true,"locked":false,"owner":null}));
            }
            let parts: Vec<&str> = out.splitn(2, '|').collect();
            let editable = parts.first().map(|s| s.trim() == "1").unwrap_or(true);
            let owner = parts.get(1).map(|s| s.trim()).filter(|s| !s.is_empty());
            ok_json(serde_json::json!({
                "success": true,
                "controlled": true,
                "editable": editable,
                "locked": !editable,
                "owner": owner,
            }))
        }

        "menu" => {
            let mut actions = vec![];
            for &item in KNOWN_MENU_ITEMS {
                let code = format!(
                    "set enabled=0 set displayName=\"{}\" set sc=##class(%Studio.SourceControl.Base).OnMenuItem(\"%SourceMenu,{}\",\"{}\",\"\",.enabled,.displayName) write enabled_\"|\"_displayName",
                    item,
                    item,
                    doc.replace('"', "\\\"")
                );
                let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
                let parts: Vec<&str> = out.splitn(2, '|').collect();
                let enabled = parts.first().map(|s| s.trim() == "1").unwrap_or(false);
                if enabled {
                    let label = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_else(|| item.to_string());
                    actions.push(serde_json::json!({"id": item, "label": label, "enabled": true}));
                }
            }
            ok_json(serde_json::json!({"success": true, "document": doc, "actions": actions}))
        }

        "checkout" => {
            let code = format!(
                "set action=0 set target=\"\" set msg=\"\" set reload=0 set sc=##class(%Studio.SourceControl.Base).UserAction(0,\"%SourceMenu,CheckOut\",\"{}\",\"\",.action,.target,.msg,.reload) write action_\"|\"_msg",
                doc.replace('"', "\\\"")
            );
            let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
            let parts: Vec<&str> = out.splitn(2, '|').collect();
            let action_code = parts.first().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
            let msg = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();

            if action_code == 0 {
                return ok_json(serde_json::json!({"success": true, "document": doc, "editable": true}));
            }
            // action=1: need user confirmation
            let eid = elicitation_store.insert(doc, ElicitationAction::ScmExecute, None, Some("CheckOut".to_string()), ns.clone());
            ok_json(serde_json::json!({
                "success": false,
                "elicitation_required": true,
                "elicitation_id": eid,
                "message": if msg.is_empty() { format!("Check out {} ?", doc) } else { msg },
                "options": ["yes", "no"],
            }))
        }

        "execute" => {
            let action_id = p.action_id.as_deref().unwrap_or("");
            let code = format!(
                "set action=0 set target=\"\" set msg=\"\" set reload=0 set sc=##class(%Studio.SourceControl.Base).UserAction(0,\"%SourceMenu,{}\",\"{}\",\"\",.action,.target,.msg,.reload) write action_\"|\"_msg",
                action_id.replace('"', "\\\""),
                doc.replace('"', "\\\"")
            );
            let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
            let parts: Vec<&str> = out.splitn(2, '|').collect();
            let action_code = parts.first().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
            let msg = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();

            match action_code {
                0 => ok_json(serde_json::json!({"success": true, "document": doc, "action_id": action_id})),
                1 => {
                    // Yes/No confirmation
                    let eid = elicitation_store.insert(doc, ElicitationAction::ScmExecute, None, Some(action_id.to_string()), ns.clone());
                    ok_json(serde_json::json!({
                        "success": false, "elicitation_required": true, "elicitation_id": eid,
                        "message": if msg.is_empty() { format!("Execute {} on {}?", action_id, doc) } else { msg },
                        "options": ["yes", "no"],
                    }))
                }
                7 => {
                    // Text prompt
                    let eid = elicitation_store.insert(doc, ElicitationAction::ScmExecute, None, Some(action_id.to_string()), ns.clone());
                    ok_json(serde_json::json!({
                        "success": false, "elicitation_required": true, "elicitation_id": eid,
                        "message": if msg.is_empty() { format!("Enter value for {}:", action_id) } else { msg },
                        "input_type": "text",
                    }))
                }
                _ => err_json("SCM_ERROR", &format!("Unexpected action code {} from UserAction", action_code)),
            }
        }

        other => err_json("INVALID_PARAM", &format!("Unknown action='{}'. Use: status, menu, checkout, execute", other)),
    }
}
