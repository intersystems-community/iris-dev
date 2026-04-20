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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IrisDocParams {
    /// Operation: get=fetch source, put=write, delete=remove, head=check existence
    pub mode: DocMode,
    /// Document name e.g. 'MyApp.Patient.cls' (required for single-doc ops)
    pub name: Option<String>,
    /// Multiple document names for batch get/delete
    #[serde(default)]
    pub names: Vec<String>,
    /// Source content (required for mode=put)
    pub content: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_namespace() -> String { "USER".to_string() }

use crate::iris::connection::IrisConnection;

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}

pub async fn handle_iris_doc(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
    source_control: bool,
    skip_source_control: bool,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    match p.mode {
        DocMode::Get => handle_get(iris, client, p).await,
        DocMode::Put => handle_put(iris, client, p, source_control, skip_source_control).await,
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
            let url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
            let req = client.get(&url).basic_auth(&iris.username, Some(&iris.password)).send();
            futs.push((name.clone(), req));
        }
        for (name, fut) in futs {
            match fut.await {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value = resp.json().await.unwrap_or_default();
                    let content = doc_content_to_string(&body);
                    results.push(serde_json::json!({"name": name, "content": content}));
                }
                Ok(resp) => results.push(serde_json::json!({"name": name, "error": format!("HTTP {}", resp.status())})),
                Err(e) => results.push(serde_json::json!({"name": name, "error": e.to_string()})),
            }
        }
        return ok_json(serde_json::json!({"success": true, "documents": results}));
    }

    let name = p.name.as_deref().unwrap_or("");
    let url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
    let resp = client.get(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send().await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if resp.status().as_u16() == 404 {
        return err_json("NOT_FOUND", &format!("Document not found: {name}"));
    }
    if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    let content = doc_content_to_string(&body);
    let ts = body["result"]["content"][0]["ts"].as_str().unwrap_or("").to_string();
    ok_json(serde_json::json!({"success": true, "name": name, "content": content, "timestamp": ts}))
}

async fn handle_put(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: IrisDocParams,
    source_control: bool,
    skip_source_control: bool,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let name = p.name.as_deref().unwrap_or("");

    // Atelier requires MAC routines to start with "ROUTINE <name>"
    // and INC files with "ROUTINE <name> [Type=INC]" — inject header if missing.
    let raw_content = p.content.as_deref().unwrap_or("");
    let content_owned;
    let content = {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        let routine_name = name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name);
        let upper = raw_content.trim_start().to_uppercase();
        if ext == "mac" && !upper.starts_with("ROUTINE ") {
            content_owned = format!("ROUTINE {}\n{}", routine_name, raw_content);
            content_owned.as_str()
        } else if ext == "inc" && !upper.starts_with("ROUTINE ") {
            content_owned = format!("ROUTINE {} [Type=INC]\n{}", routine_name, raw_content);
            content_owned.as_str()
        } else {
            content_owned = String::new();
            raw_content
        }
    };

    // SCM OnBeforeSave hook
    if source_control {
        let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
        let scm_code = format!(
            "set sc=##class(%Studio.SourceControl.ISC).OnBeforeSave(\"{name}\") if $system.Status.IsError(sc) {{ write \"SCM_ERROR:\",$system.Status.GetErrorText(sc) }} else {{ write \"SCM_OK\" }}"
        );
        if let Ok(resp) = client.post(&xecute_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"expression": scm_code}))
            .send().await
        {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let out = body["result"]["content"][0]["content"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if out.starts_with("SCM_ERROR:") {
                    return err_json("SCM_REJECTED", &out.replace("SCM_ERROR:", ""));
                }
            }
        }
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
    if skip_source_control {
        url.push_str("?csp=1");
    }

    let resp = client.put(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .json(&serde_json::json!({"enc": false, "content": lines}))
        .send().await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    if resp.status().as_u16() == 409 {
        // ETag conflict — fetch current ETag and retry once
        let head_url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
        let etag = client.head(&head_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .send().await
            .ok()
            .and_then(|r| r.headers().get("ETag").and_then(|v| v.to_str().ok()).map(|s| s.to_string()));

        let retry_resp = client.put(&url)
            .basic_auth(&iris.username, Some(&iris.password))
            .header("If-None-Match", etag.as_deref().unwrap_or(""))
            .json(&serde_json::json!({"enc": false, "content": lines}))
            .send().await
            .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error on retry: {e}"), None))?;

        if !retry_resp.status().is_success() {
            return err_json("CONFLICT", "Document modified by another user; retry failed");
        }
    } else if !resp.status().is_success() {
        return err_json("IRIS_UNREACHABLE", &format!("HTTP {}", resp.status()));
    }

    // SCM OnAfterSave (best-effort)
    if source_control {
        let xecute_url = iris.atelier_url(&format!("/v1/{}/action/xecute", p.namespace));
        let _ = client.post(&xecute_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .json(&serde_json::json!({"expression": format!("do ##class(%Studio.SourceControl.ISC).OnAfterSave(\"{name}\")")}))
            .send().await;
    }

    ok_json(serde_json::json!({"success": true, "name": name}))
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
            let url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
            match client.delete(&url).basic_auth(&iris.username, Some(&iris.password)).send().await {
                Ok(r) if r.status().is_success() => deleted.push(name.clone()),
                Ok(r) => errors.push(serde_json::json!({"name": name, "error": format!("HTTP {}", r.status())})),
                Err(e) => errors.push(serde_json::json!({"name": name, "error": e.to_string()})),
            }
        }
        return ok_json(serde_json::json!({"success": errors.is_empty(), "deleted": deleted, "errors": errors}));
    }

    let name = p.name.as_deref().unwrap_or("");
    let url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
    let resp = client.delete(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send().await
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
    let url = iris.atelier_url(&format!("/v8/{}/doc/{}", p.namespace, urlencoding::encode(name)));
    let resp = client.head(&url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send().await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("HTTP error: {e}"), None))?;

    let exists = resp.status().is_success();
    let ts = resp.headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    ok_json(serde_json::json!({"success": true, "name": name, "exists": exists, "timestamp": ts}))
}

fn doc_content_to_string(body: &serde_json::Value) -> String {
    body["result"]["content"][0]["content"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}
