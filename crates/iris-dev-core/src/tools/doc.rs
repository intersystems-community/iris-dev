//! iris_doc — document CRUD via Atelier REST v8.
//! Handles get/put/delete/head with ETag conflict retry and optional SCM hooks.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DocMode {
    Get,
    Put,
    Delete,
    Head,
}

fn default_mode() -> DocMode {
    DocMode::Get
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IrisDocParams {
    /// Operation: get=fetch source, put=write, delete=remove, head=check existence. Defaults to "get".
    #[serde(default = "default_mode", alias = "action")]
    pub mode: DocMode,
    /// Document name e.g. 'MyApp.Patient.cls'
    #[serde(alias = "document")]
    pub name: Option<String>,
    /// Multiple document names for batch get/delete
    #[serde(default)]
    pub names: Vec<String>,
    /// Source content (required for mode=put)
    pub content: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Elicitation resume ID (from a prior elicitation_required response)
    pub elicitation_id: Option<String>,
    /// User's answer to the elicitation question ("yes" or "no")
    pub elicitation_answer: Option<String>,
}

fn default_namespace() -> String {
    "USER".to_string()
}
use crate::iris::connection::IrisConnection;

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![
        rmcp::model::Content::text(v.to_string()),
    ]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}

pub async fn handle_iris_doc(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
    elicitation_store: &crate::elicitation::ElicitationStore,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    match p.mode {
        DocMode::Get => handle_get(iris, client, p).await,
        DocMode::Put => handle_put(iris, client, p, elicitation_store).await,
        DocMode::Delete => handle_delete(iris, client, p).await,
        DocMode::Head => handle_head(iris, client, p).await,
    }
}

async fn handle_get(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    // Batch get
    if !p.names.is_empty() {
        let mut results = vec![];
        let mut futs = vec![];
        for name in &p.names {
            let url =
                iris.versioned_ns_url(&p.namespace, &format!("/doc/{}", urlencoding::encode(name)));
            let req = client
                .get(&url)
                .basic_auth(&iris.username, Some(&iris.password))
                .send();
            futs.push((name.clone(), req));
        }
        for (name, fut) in futs {
            match fut.await {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value = resp.json().await.unwrap_or_default();
                    let content = doc_content_to_string(&body);
                    results.push(serde_json::json!({"name": name, "content": content}));
                }
                Ok(resp) => results.push(
                    serde_json::json!({"name": name, "error": format!("HTTP {}", resp.status())}),
                ),
                Err(e) => results.push(serde_json::json!({"name": name, "error": e.to_string()})),
            }
        }
        return ok_json(serde_json::json!({"success": true, "documents": results}));
    }

    let name = p.name.as_deref().unwrap_or("");
    let url = iris.versioned_ns_url(&p.namespace, &format!("/doc/{}", urlencoding::encode(name)));
    let resp = client
        .get(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if resp.status().as_u16() == 404 {
        return err_json("NOT_FOUND", &format!("Document not found: {name}"));
    }
    if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    let content = doc_content_to_string(&body);
    let ts = body["result"]["content"][0]["ts"]
        .as_str()
        .unwrap_or("")
        .to_string();
    ok_json(serde_json::json!({"success": true, "name": name, "content": content, "timestamp": ts}))
}

async fn handle_put(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
    elicitation_store: &crate::elicitation::ElicitationStore,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let name = p.name.as_deref().unwrap_or("");
    let ns = &p.namespace;

    // Elicitation resume — user answered a prior SCM dialog
    if let (Some(eid), Some(answer)) = (&p.elicitation_id, &p.elicitation_answer) {
        if let Some(pending) = elicitation_store.lookup(eid) {
            elicitation_store.clear(eid);
            if answer.to_lowercase() != "yes" {
                return ok_json(
                    serde_json::json!({"success": false, "error_code": "WRITE_ABORTED", "error": "User declined checkout"}),
                );
            }
            // User said yes — proceed with the stored content directly
            let resume_content = pending.content.as_deref().unwrap_or("");
            return do_write(
                iris,
                client,
                &pending.document,
                resume_content,
                &pending.namespace,
            )
            .await;
        }
        return err_json(
            "ELICITATION_EXPIRED",
            "Elicitation session expired or not found",
        );
    }

    // Inject ROUTINE header for .mac/.inc if missing
    let raw_content = p.content.as_deref().unwrap_or("");
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let routine_name = name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name);
    let needs_header = !raw_content
        .trim_start()
        .to_uppercase()
        .starts_with("ROUTINE ");
    let content_owned: String;
    let content: &str = match ext.as_str() {
        "mac" if needs_header => {
            content_owned = format!("ROUTINE {}\n{}", routine_name, raw_content);
            &content_owned
        }
        "inc" if needs_header => {
            content_owned = format!("ROUTINE {} [Type=INC]\n{}", routine_name, raw_content);
            &content_owned
        }
        _ => raw_content,
    };

    // SCM OnBeforeSave — check if write is allowed (requires docker exec; skipped if unavailable)
    let scm_check = format!(
        "set scmObj=##class(%Studio.SourceControl.Base).%GetImplementationObject(\"{n}\") if '$IsObject(scmObj) {{ write \"NO_SCM\" }} else {{ set action=0 set msg=\"\" set target=\"\" set reload=0 set sc=scmObj.UserAction(0,\"%SourceMenu,CheckOut\",\"{n}\",\"\",.action,.target,.msg,.reload) write action_\"|\"_msg }}",
        n = name.replace('"', "\\\"")
    );
    if let Ok(out) = iris.execute(&scm_check, ns).await {
        let out = out.trim().to_string();
        if out != "NO_SCM" && !out.is_empty() {
            let parts: Vec<&str> = out.splitn(2, '|').collect();
            let action_code = parts
                .first()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(0);
            let msg = parts.get(1).map(|s| s.trim()).unwrap_or("");

            if action_code == 1 {
                let eid = elicitation_store.insert(
                    name,
                    crate::elicitation::ElicitationAction::Put,
                    Some(content.to_string()),
                    None,
                    ns.clone(),
                );
                return ok_json(serde_json::json!({
                    "success": false,
                    "elicitation_required": true,
                    "elicitation_id": eid,
                    "message": if msg.is_empty() { format!("{} requires checkout. Check out and write?", name) } else { msg.to_string() },
                    "options": ["yes", "no"],
                }));
            } else if action_code == 6 {
                return err_json("SCM_REJECTED", &format!("Source control rejected: {}", msg));
            }
            // action_code == 0: proceed
        }
    }

    do_write(iris, client, name, content, ns).await
}

async fn do_write(
    iris: &IrisConnection,
    client: &reqwest::Client,
    name: &str,
    content: &str,
    namespace: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let lines: Vec<&str> = content.lines().collect();
    let url = iris.versioned_ns_url(namespace, &format!("/doc/{}", urlencoding::encode(name)));

    let resp = client
        .put(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .json(&serde_json::json!({"enc": false, "content": lines}))
        .send()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if resp.status().as_u16() == 409 {
        let head_url =
            iris.versioned_ns_url(namespace, &format!("/doc/{}", urlencoding::encode(name)));
        let etag = client
            .head(&head_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get("ETag")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
            });

        let retry = client
            .put(&url)
            .basic_auth(&iris.username, Some(&iris.password))
            .header("If-None-Match", etag.as_deref().unwrap_or(""))
            .json(&serde_json::json!({"enc": false, "content": lines}))
            .send()
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP retry error: {e}"), None))?;

        if !retry.status().is_success() {
            return err_json(
                "CONFLICT",
                "Document modified by another user; retry failed",
            );
        }
    } else if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()));
    }

    // Write open hint for VS Code auto-open
    crate::tools::write_open_hint(namespace, name);

    let open_uri = format!("isfs://{}/{}", namespace, name);
    ok_json(serde_json::json!({"success": true, "name": name, "open_uri": open_uri}))
}

async fn handle_delete(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    // Batch delete
    if !p.names.is_empty() {
        let mut deleted = vec![];
        let mut errors = vec![];
        for name in &p.names {
            let url =
                iris.versioned_ns_url(&p.namespace, &format!("/doc/{}", urlencoding::encode(name)));
            match client
                .delete(&url)
                .basic_auth(&iris.username, Some(&iris.password))
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => deleted.push(name.clone()),
                Ok(r) => errors.push(
                    serde_json::json!({"name": name, "error": format!("HTTP {}", r.status())}),
                ),
                Err(e) => errors.push(serde_json::json!({"name": name, "error": e.to_string()})),
            }
        }
        return ok_json(
            serde_json::json!({"success": errors.is_empty(), "deleted": deleted, "errors": errors}),
        );
    }

    let name = p.name.as_deref().unwrap_or("");
    let url = iris.versioned_ns_url(&p.namespace, &format!("/doc/{}", urlencoding::encode(name)));
    let resp = client
        .delete(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if resp.status().as_u16() == 404 {
        return err_json("NOT_FOUND", &format!("Document not found: {name}"));
    }
    if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()));
    }
    ok_json(serde_json::json!({"success": true, "name": name}))
}

async fn handle_head(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let name = p.name.as_deref().unwrap_or("");
    let url = iris.versioned_ns_url(&p.namespace, &format!("/doc/{}", urlencoding::encode(name)));
    let resp = client
        .head(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    let exists = resp.status().is_success();
    let ts = resp
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    ok_json(serde_json::json!({"success": true, "name": name, "exists": exists, "timestamp": ts}))
}

fn doc_content_to_string(body: &serde_json::Value) -> String {
    body["result"]["content"][0]["content"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
