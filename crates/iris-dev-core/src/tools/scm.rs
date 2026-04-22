//! iris_source_control — SCM status, menu, checkout, execute via Atelier xecute.

use crate::elicitation::{ElicitationAction, ElicitationStore};
use crate::iris::connection::IrisConnection;
use schemars::JsonSchema;
use serde::Deserialize;

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![
        rmcp::model::Content::text(v.to_string()),
    ]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn default_namespace() -> String {
    "USER".to_string()
}

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
    let resp = client
        .post(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .json(&serde_json::json!({"expression": code}))
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await?;
    Ok(body["result"]["content"][0]["content"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default())
}

/// Escape a string for safe interpolation into an ObjectScript double-quoted literal.
fn os_quote(s: &str) -> String {
    s.replace('"', "\\\"")
}

/// Parse "code|msg" output from SCM xecute helpers. Returns (action_code, msg).
fn parse_action_msg(out: &str) -> (u8, &str) {
    let mut parts = out.splitn(2, '|');
    let code = parts
        .next()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .unwrap_or(0);
    let msg = parts.next().map(str::trim).unwrap_or("");
    (code, msg)
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
        let Some(pending) = elicitation_store.lookup(eid) else {
            return err_json(
                "ELICITATION_EXPIRED",
                "Elicitation session expired or not found",
            );
        };
        elicitation_store.clear(eid);
        let action_id = pending.scm_action_id.as_deref().unwrap_or("");
        let after_code = format!(
            "set sc=##class(%Studio.SourceControl.Base).AfterUserAction(0,\"{}\",\"{}\",{},\"{}\") write $system.Status.GetErrorText(sc)",
            os_quote(action_id),
            os_quote(&pending.document),
            if answer == "yes" { "1" } else { "0" },
            os_quote(answer),
        );
        let out = xecute(iris, client, &after_code, &pending.namespace)
            .await
            .unwrap_or_default();
        if out.is_empty() || out.starts_with('$') {
            return ok_json(
                serde_json::json!({"success": true, "document": pending.document, "action_id": action_id}),
            );
        }
        return err_json("SCM_ERROR", &out);
    }

    match p.action.as_str() {
        "status" => {
            // Check if SCM is installed
            let doc_q = os_quote(doc);
            let check_code = format!(
                "set obj=##class(%Studio.SourceControl.Base).%GetImplementationObject(\"{doc_q}\") if '$IsObject(obj) {{ write \"UNCONTROLLED\" }} else {{ set editable=obj.IsEditable(\"{doc_q}\") write editable_\"|\"_$get(obj.Owner) }}"
            );
            let out = xecute(iris, client, &check_code, ns)
                .await
                .unwrap_or_else(|_| "UNCONTROLLED".to_string());
            if out.trim() == "UNCONTROLLED" || out.is_empty() {
                return ok_json(
                    serde_json::json!({"success":true,"controlled":false,"editable":true,"locked":false,"owner":null}),
                );
            }
            let (editable_flag, owner) = parse_action_msg(&out);
            let editable = editable_flag == 1;
            let owner = Some(owner).filter(|s| !s.is_empty());
            ok_json(serde_json::json!({
                "success": true,
                "controlled": true,
                "editable": editable,
                "locked": !editable,
                "owner": owner,
            }))
        }

        "menu" => {
            let doc_q = os_quote(doc);
            let mut actions = vec![];
            for &item in KNOWN_MENU_ITEMS {
                let code = format!(
                    "set enabled=0 set displayName=\"{item}\" set sc=##class(%Studio.SourceControl.Base).OnMenuItem(\"%SourceMenu,{item}\",\"{doc_q}\",\"\",.enabled,.displayName) write enabled_\"|\"_displayName"
                );
                let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
                let (enabled_flag, label) = parse_action_msg(&out);
                if enabled_flag == 1 {
                    let label = if label.is_empty() {
                        item.to_string()
                    } else {
                        label.to_string()
                    };
                    actions.push(serde_json::json!({"id": item, "label": label, "enabled": true}));
                }
            }
            ok_json(serde_json::json!({"success": true, "document": doc, "actions": actions}))
        }

        "checkout" => {
            let code = user_action_code("CheckOut", doc);
            let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
            let (action_code, msg) = parse_action_msg(&out);

            if action_code == 0 {
                return ok_json(
                    serde_json::json!({"success": true, "document": doc, "editable": true}),
                );
            }
            // action=1: need user confirmation
            let eid = elicitation_store.insert(
                doc,
                ElicitationAction::ScmExecute,
                None,
                Some("CheckOut".to_string()),
                ns.clone(),
            );
            ok_json(serde_json::json!({
                "success": false,
                "elicitation_required": true,
                "elicitation_id": eid,
                "message": if msg.is_empty() { format!("Check out {} ?", doc) } else { msg.to_string() },
                "options": ["yes", "no"],
            }))
        }

        "execute" => {
            let action_id = p.action_id.as_deref().unwrap_or("");
            let code = user_action_code(action_id, doc);
            let out = xecute(iris, client, &code, ns).await.unwrap_or_default();
            let (action_code, msg) = parse_action_msg(&out);

            match action_code {
                0 => ok_json(
                    serde_json::json!({"success": true, "document": doc, "action_id": action_id}),
                ),
                1 => {
                    // Yes/No confirmation
                    let eid = elicitation_store.insert(
                        doc,
                        ElicitationAction::ScmExecute,
                        None,
                        Some(action_id.to_string()),
                        ns.clone(),
                    );
                    ok_json(serde_json::json!({
                        "success": false, "elicitation_required": true, "elicitation_id": eid,
                        "message": if msg.is_empty() { format!("Execute {} on {}?", action_id, doc) } else { msg.to_string() },
                        "options": ["yes", "no"],
                    }))
                }
                7 => {
                    // Text prompt
                    let eid = elicitation_store.insert(
                        doc,
                        ElicitationAction::ScmExecute,
                        None,
                        Some(action_id.to_string()),
                        ns.clone(),
                    );
                    ok_json(serde_json::json!({
                        "success": false, "elicitation_required": true, "elicitation_id": eid,
                        "message": if msg.is_empty() { format!("Enter value for {}:", action_id) } else { msg.to_string() },
                        "input_type": "text",
                    }))
                }
                _ => err_json(
                    "SCM_ERROR",
                    &format!("Unexpected action code {} from UserAction", action_code),
                ),
            }
        }

        other => err_json(
            "INVALID_PARAM",
            &format!(
                "Unknown action='{}'. Use: status, menu, checkout, execute",
                other
            ),
        ),
    }
}

/// Build the ObjectScript snippet that invokes `%Studio.SourceControl.Base:UserAction`
/// for a given menu item id and document, writing "action|msg" to the output stream.
fn user_action_code(action_id: &str, doc: &str) -> String {
    format!(
        "set action=0 set target=\"\" set msg=\"\" set reload=0 set sc=##class(%Studio.SourceControl.Base).UserAction(0,\"%SourceMenu,{}\",\"{}\",\"\",.action,.target,.msg,.reload) write action_\"|\"_msg",
        os_quote(action_id),
        os_quote(doc),
    )
}
