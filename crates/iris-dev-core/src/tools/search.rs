//! iris_search — full-text search via Atelier REST v2 with sync→async fallback.

use crate::iris::connection::IrisConnection;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    /// Filter to document category: CLS, MAC, INT, INC, or ALL (default)
    pub category: Option<String>,
    /// Wildcard document scopes e.g. ["HS.FHIR.*.cls"]
    #[serde(default)]
    pub documents: Vec<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_namespace() -> String {
    "USER".to_string()
}

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![
        rmcp::model::Content::text(v.to_string()),
    ]))
}

pub async fn handle_iris_search(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: SearchParams,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let category = p.category.as_deref().unwrap_or("ALL");
    let mut query_string = format!(
        "query={}&regex={}&sys=false&category={}",
        urlencoding::encode(&p.query),
        p.regex,
        category,
    );
    if p.case_sensitive {
        query_string.push_str("&case=1");
    }

    let sync_url =
        iris.versioned_ns_url(&p.namespace, &format!("/action/search?{}", query_string));

    // Try sync search with 2s timeout
    let sync_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| client.clone());

    let sync_result = sync_client
        .get(&sync_url)
        .basic_auth(&iris.username, Some(&iris.password))
        .send()
        .await;

    match sync_result {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            // If we got a workId, it's async — fall through to polling
            if body["result"]["workId"].is_null() {
                return parse_search_results(body, &p.query);
            }
            let work_id = body["result"]["workId"].as_str().unwrap_or("").to_string();
            poll_async_search(iris, client, &work_id, &p.namespace, &p.query).await
        }
        _ => {
            // Timeout or error — fall back to async POST
            let post_url = iris.versioned_ns_url(&p.namespace, "/action/search");
            let post_body = serde_json::json!({
                "query": p.query,
                "regex": p.regex,
                "sys": false,
                "category": category,
            });
            let resp = client
                .post(&post_url)
                .basic_auth(&iris.username, Some(&iris.password))
                .json(&post_body)
                .send()
                .await
                .map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("Search request failed: {e}"), None)
                })?;

            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if let Some(work_id) = body["result"]["workId"].as_str() {
                poll_async_search(iris, client, work_id, &p.namespace, &p.query).await
            } else {
                parse_search_results(body, &p.query)
            }
        }
    }
}

async fn poll_async_search(
    iris: &IrisConnection,
    client: &reqwest::Client,
    work_id: &str,
    namespace: &str,
    query: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let poll_url = iris.versioned_ns_url(
        namespace,
        &format!("/action/search?workId={}", urlencoding::encode(work_id)),
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        if std::time::Instant::now() > deadline {
            return ok_json(serde_json::json!({
                "success": false,
                "error_code": "SEARCH_TIMEOUT",
                "error": "Async search did not complete within 5 minutes",
                "query": query,
            }));
        }

        let resp = client
            .get(&poll_url)
            .basic_auth(&iris.username, Some(&iris.password))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                if body["result"]["workId"].is_null() {
                    return parse_search_results(body, query);
                }
                // Still pending — keep polling
            }
            _ => continue,
        }
    }
}

fn parse_search_results(
    body: serde_json::Value,
    query: &str,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let content = body["result"]["content"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let total = content.len();
    let truncated = total > 200;
    let results: Vec<serde_json::Value> = content
        .into_iter()
        .take(200)
        .map(|item| {
            serde_json::json!({
                "document": item["doc"],
                "line": item["atLine"],
                "member": item["member"],
                "content": item["text"],
            })
        })
        .collect();

    ok_json(serde_json::json!({
        "success": true,
        "query": query,
        "results": results,
        "total_found": total,
        "truncated": truncated,
    }))
}
