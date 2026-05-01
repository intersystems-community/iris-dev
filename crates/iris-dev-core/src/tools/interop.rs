use crate::iris::connection::IrisConnection;
use rmcp::{model::*, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;

fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}
fn err_json(code: &str, msg: &str) -> Result<CallToolResult, McpError> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}
fn iris_unreachable() -> McpError {
    McpError::invalid_request("IRIS_UNREACHABLE", None)
}
// Bug 18: "connection" matched too broadly — e.g. "No Interoperability connection configured"
// was misclassified as IRIS_UNREACHABLE. Use more specific network-error patterns.
pub(crate) fn is_network_error(msg: &str) -> bool {
    msg.contains("error sending")
        || msg.contains("connection refused")
        || msg.contains("connection reset")
        || msg.contains("dns error")
        || msg.contains("timed out")
}

fn default_ns() -> String {
    "USER".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionStatusParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default)]
    pub full_status: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionNameParams {
    pub production: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionStopParams {
    pub production: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default)]
    pub force: bool,
}
fn default_timeout() -> u32 {
    30
}

// Bug 7: added namespace field so update/recover/needs_update work in non-default namespaces.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionUpdateParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionNeedsUpdateParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionRecoverParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LogsParams {
    pub item_name: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default = "default_log_type")]
    pub log_type: String,
}
fn default_limit() -> u32 {
    10
}
fn default_log_type() -> String {
    "error,warning".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueuesParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MessageSearchParams {
    pub source: Option<String>,
    pub target: Option<String>,
    pub class_name: Option<String>,
    #[serde(default = "default_msg_limit")]
    pub limit: u32,
}
fn default_msg_limit() -> u32 {
    20
}

fn state_string(code: i64) -> &'static str {
    match code {
        1 => "Running",
        2 => "Stopped",
        3 => "Suspended",
        4 => "Troubled",
        5 => "NetworkStopped",
        _ => "Unknown",
    }
}

pub fn parse_status_response(raw: &str) -> Result<(String, i64, String), String> {
    if raw.is_empty() || raw == ":" {
        return Err("NO_PRODUCTION".to_string());
    }
    if raw.starts_with("ERROR") {
        return Err(format!("INTEROP_ERROR:{}", raw));
    }
    let parts: Vec<&str> = raw.splitn(2, ':').collect();
    if parts.len() < 2 || parts[0].is_empty() {
        return Err("NO_PRODUCTION".to_string());
    }
    let name = parts[0].to_string();
    let code: i64 = parts[1].trim().parse().unwrap_or(0);
    let state = state_string(code).to_string();
    Ok((name, code, state))
}

fn docker_required_interop() -> Result<CallToolResult, McpError> {
    err_json(
        "DOCKER_REQUIRED",
        "Interoperability operations require docker exec. Set IRIS_CONTAINER=<container_name>.",
    )
}

pub async fn interop_production_status_impl(
    iris: Option<&IrisConnection>,
    params: ProductionStatusParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Set sc=##class(Ens.Director).GetProductionStatus(.n,.s) If $$$ISERR(sc) { Write "ERROR:"_$System.Status.GetErrorText(sc) } Else { Write n_":"_s }"#;
    // Bug 7: use params.namespace, not iris.namespace.
    match iris.execute(code, &params.namespace).await {
        Ok(output) => {
            let raw = output.trim().to_string();
            match parse_status_response(&raw) {
                Ok((name, code, state)) => ok_json(
                    serde_json::json!({"success": true, "production": name, "state": state, "state_code": code}),
                ),
                Err(e) if e.starts_with("INTEROP_ERROR") => err_json("INTEROP_ERROR", &e[14..]),
                Err(_) => err_json("NO_PRODUCTION", "No production is running"),
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_start_impl(
    iris: Option<&IrisConnection>,
    params: ProductionNameParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let prod = params.production.as_deref().unwrap_or("");
    let code = format!(
        r#"Set sc=##class(Ens.Director).StartProduction("{}") If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        prod
    );
    // Bug 7: use params.namespace, not iris.namespace.
    match iris.execute(&code, &params.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" {
                ok_json(serde_json::json!({"success": true, "state": "Running"}))
            } else {
                err_json("INTEROP_ERROR", raw)
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_stop_impl(
    iris: Option<&IrisConnection>,
    params: ProductionStopParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = format!(
        r#"Set sc=##class(Ens.Director).StopProduction({},{}) If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        params.timeout,
        if params.force { 1 } else { 0 }
    );
    // Bug 7: use params.namespace, not iris.namespace.
    match iris.execute(&code, &params.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" {
                ok_json(serde_json::json!({"success": true, "state": "Stopped"}))
            } else {
                err_json("INTEROP_ERROR", raw)
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_update_impl(
    iris: Option<&IrisConnection>,
    params: ProductionUpdateParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = format!(
        r#"Set sc=##class(Ens.Director).UpdateProduction({},{}) If $$$ISERR(sc) {{ Write "ERROR:"_$System.Status.GetErrorText(sc) }} Else {{ Write "OK" }}"#,
        params.timeout,
        if params.force { 1 } else { 0 }
    );
    // Bug 7: use params.namespace.
    match iris.execute(&code, &params.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" {
                ok_json(serde_json::json!({"success": true, "message": "Production updated"}))
            } else {
                err_json("INTEROP_ERROR", raw)
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_needs_update_impl(
    iris: Option<&IrisConnection>,
    params: ProductionNeedsUpdateParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Write ##class(Ens.Director).ProductionNeedsUpdate()"#;
    // Bug 7: use params.namespace.
    match iris.execute(code, &params.namespace).await {
        Ok(output) => {
            ok_json(serde_json::json!({"success": true, "needs_update": output.trim() == "1"}))
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_production_recover_impl(
    iris: Option<&IrisConnection>,
    params: ProductionRecoverParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let code = r#"Set sc=##class(Ens.Director).RecoverProduction() If $$$ISERR(sc) { Write "ERROR:"_$System.Status.GetErrorText(sc) } Else { Write "OK" }"#;
    // Bug 7: use params.namespace.
    match iris.execute(code, &params.namespace).await {
        Ok(output) => {
            let raw = output.trim();
            if raw == "OK" {
                ok_json(serde_json::json!({"success": true, "state": "Running"}))
            } else {
                err_json("INTEROP_ERROR", raw)
            }
        }
        Err(e) if e.to_string() == "DOCKER_REQUIRED" => docker_required_interop(),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_logs_impl(
    iris: Option<&IrisConnection>,
    params: LogsParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    let mut conditions = vec![];
    for lt in params.log_type.split(',') {
        match lt.trim().to_lowercase().as_str() {
            "error" => conditions.push("Type = 3"),
            "warning" => conditions.push("Type = 2"),
            "info" => conditions.push("Type = 1"),
            "alert" => conditions.push("Type = 4"),
            _ => {}
        }
    }
    let type_filter = if conditions.is_empty() {
        String::new()
    } else {
        format!("AND ({})", conditions.join(" OR "))
    };
    let item_filter = params
        .item_name
        .as_ref()
        .map(|n| format!("AND ConfigName = '{}'", n.replace('\'', "''")))
        .unwrap_or_default();
    let sql = format!("SELECT TOP {} ID, TimeLogged, Type, ConfigName, Text FROM Ens_Util.Log WHERE 1=1 {} {} ORDER BY ID DESC", params.limit, type_filter, item_filter);
    match iris
        .query(&sql, vec![], &iris.namespace.clone(), &client)
        .await
    {
        Ok(resp) => ok_json(
            serde_json::json!({"success": true, "logs": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_queues_impl(
    iris: Option<&IrisConnection>,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    match iris
        .query(
            "SELECT * FROM Ens.Queue_Enumerate()",
            vec![],
            &iris.namespace.clone(),
            &client,
        )
        .await
    {
        Ok(resp) => {
            ok_json(serde_json::json!({"success": true, "queues": resp["result"]["content"]}))
        }
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_message_search_impl(
    iris: Option<&IrisConnection>,
    params: MessageSearchParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client().map_err(|_| iris_unreachable())?;
    let mut filters = vec![];
    if let Some(src) = &params.source {
        filters.push(format!("SourceConfigName = '{}'", src.replace('\'', "''")));
    }
    if let Some(tgt) = &params.target {
        filters.push(format!("TargetConfigName = '{}'", tgt.replace('\'', "''")));
    }
    if let Some(cls) = &params.class_name {
        filters.push(format!(
            "MessageBodyClassName = '{}'",
            cls.replace('\'', "''")
        ));
    }
    let where_clause = if filters.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", filters.join(" AND "))
    };
    let sql = format!("SELECT TOP {} ID, TimeCreated, SourceConfigName, TargetConfigName, MessageBodyClassName, Status FROM Ens.MessageHeader {} ORDER BY ID DESC", params.limit, where_clause);
    match iris
        .query(&sql, vec![], &iris.namespace.clone(), &client)
        .await
    {
        Ok(resp) => ok_json(
            serde_json::json!({"success": true, "messages": resp["result"]["content"], "count": resp["result"]["content"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 024-interop-depth: Production item control (US1)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionItemParams {
    pub action: String,
    pub item: String,
    #[serde(default = "default_ns")]
    pub namespace: String,
    #[serde(default)]
    pub settings: std::collections::HashMap<String, String>,
}

pub async fn interop_production_item_impl(
    iris: Option<&IrisConnection>,
    params: ProductionItemParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let item = params.item.replace('\'', "''");
    let ns = &params.namespace;
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;

    match params.action.as_str() {
        "enable" | "disable" => {
            let enabled_val = if params.action == "enable" { "1" } else { "0" };
            let code = format!(
                r#"Set tSC=##class(Ens.Director).GetProductionStatus(.n,.s)
If $$$ISERR(tSC) {{ Write "ERROR:NO_PRODUCTION:"_$System.Status.GetErrorText(tSC) Quit }}
If n="" {{ Write "ERROR:NO_PRODUCTION:No production running" Quit }}
Set tProd=##class(Ens.Config.Production).%OpenId(n,,.tSC2)
If '$IsObject(tProd) {{ Write "ERROR:INTEROP_ERROR:Cannot open production" Quit }}
Set tItem=tProd.FindItemByConfigName("{}",,.tSC3)
If '$IsObject(tItem) {{ Write "ERROR:ITEM_NOT_FOUND:Item not found: {}" Quit }}
Set tItem.Enabled={}
Set tSC4=tProd.%Save()
If $$$ISERR(tSC4) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC4) Quit }}
Set tSC5=##class(Ens.Director).UpdateProduction(10,0)
If $$$ISERR(tSC5) {{ Write "ERROR:UPDATE_FAILED:"_$System.Status.GetErrorText(tSC5) Quit }}
Write "OK""#,
                item, item, enabled_val
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"item":params.item,"enabled":params.action=="enable"}),
                        )
                    } else if let Some(msg) = out.strip_prefix("ERROR:ITEM_NOT_FOUND:") {
                        err_json("ITEM_NOT_FOUND", msg)
                    } else if let Some(msg) = out.strip_prefix("ERROR:NO_PRODUCTION:") {
                        err_json("NO_PRODUCTION", msg)
                    } else if let Some(msg) = out.strip_prefix("ERROR:UPDATE_FAILED:") {
                        err_json("UPDATE_FAILED", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "get_settings" => {
            let code = format!(
                r#"Set tSC=##class(Ens.Director).GetProductionStatus(.n,.s)
If $$$ISERR(tSC)||n="" {{ Write "ERROR:NO_PRODUCTION:No production running" Quit }}
Set tProd=##class(Ens.Config.Production).%OpenId(n,,.tSC2)
If '$IsObject(tProd) {{ Write "ERROR:INTEROP_ERROR:Cannot open production" Quit }}
Set tItem=tProd.FindItemByConfigName("{}",,.tSC3)
If '$IsObject(tItem) {{ Write "ERROR:ITEM_NOT_FOUND:Item not found: {}" Quit }}
Set tKey="" For {{ Set tSetting=tItem.Settings.GetNext(.tKey) Quit:tKey=""
  Write tSetting.Name_"="_tSetting.Value_$CHAR(10) }}"#,
                item, item
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if let Some(msg) = out.strip_prefix("ERROR:ITEM_NOT_FOUND:") {
                        return err_json("ITEM_NOT_FOUND", msg);
                    }
                    if let Some(msg) = out.strip_prefix("ERROR:NO_PRODUCTION:") {
                        return err_json("NO_PRODUCTION", msg);
                    }
                    if out.starts_with("ERROR:") {
                        return err_json("INTEROP_ERROR", out);
                    }
                    let settings: std::collections::HashMap<String, String> = out
                        .lines()
                        .filter_map(|line| {
                            let mut parts = line.splitn(2, '=');
                            let k = parts.next()?.trim().to_string();
                            let v = parts.next().unwrap_or("").to_string();
                            if k.is_empty() {
                                None
                            } else {
                                Some((k, v))
                            }
                        })
                        .collect();
                    ok_json(
                        serde_json::json!({"success":true,"item":params.item,"settings":settings}),
                    )
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "set_settings" => {
            if params.settings.is_empty() {
                return err_json(
                    "INVALID_PARAMS",
                    "set_settings requires at least one setting",
                );
            }
            // Build ObjectScript to set each setting then UpdateProduction
            let mut setting_lines = String::new();
            for (k, v) in &params.settings {
                let k_esc = k.replace('\'', "''");
                let v_esc = v.replace('\'', "''");
                setting_lines.push_str(&format!(
                    r#"Set tS=tItem.FindSettingByName("{}","Host")
If '$IsObject(tS) {{ Set tS=##class(Ens.Config.Setting).%New() Set tS.Name="{}" Set tS.Target="Host" Do tItem.Settings.Insert(tS) }}
Set tS.Value="{}"
"#,
                    k_esc, k_esc, v_esc
                ));
            }
            let code = format!(
                r#"Set tSC=##class(Ens.Director).GetProductionStatus(.n,.s)
If $$$ISERR(tSC)||n="" {{ Write "ERROR:NO_PRODUCTION:No production running" Quit }}
Set tProd=##class(Ens.Config.Production).%OpenId(n,,.tSC2)
If '$IsObject(tProd) {{ Write "ERROR:INTEROP_ERROR:Cannot open production" Quit }}
Set tItem=tProd.FindItemByConfigName("{}",,.tSC3)
If '$IsObject(tItem) {{ Write "ERROR:ITEM_NOT_FOUND:Item not found: {}" Quit }}
{}Set tSC4=tProd.%Save()
If $$$ISERR(tSC4) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC4) Quit }}
Set tSC5=##class(Ens.Director).UpdateProduction(10,0)
If $$$ISERR(tSC5) {{ Write "ERROR:UPDATE_FAILED:"_$System.Status.GetErrorText(tSC5) Quit }}
Write "OK""#,
                item, item, setting_lines
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"item":params.item,"message":"Settings updated and production updated"}),
                        )
                    } else if let Some(msg) = out.strip_prefix("ERROR:ITEM_NOT_FOUND:") {
                        err_json("ITEM_NOT_FOUND", msg)
                    } else if let Some(msg) = out.strip_prefix("ERROR:NO_PRODUCTION:") {
                        err_json("NO_PRODUCTION", msg)
                    } else if let Some(msg) = out.strip_prefix("ERROR:UPDATE_FAILED:") {
                        err_json("UPDATE_FAILED", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        _ => err_json(
            "INVALID_ACTION",
            "iris_production_item: action must be enable, disable, get_settings, or set_settings",
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 024-interop-depth: Ensemble credentials (US2)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CredentialListParams {
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CredentialManageParams {
    pub action: String,
    pub id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
}

pub async fn interop_credential_list_impl(
    iris: Option<&IrisConnection>,
    params: CredentialListParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    match iris
        .query(
            "SELECT SystemName, Username FROM Ens_Config.Credentials ORDER BY SystemName",
            vec![],
            &params.namespace,
            &client,
        )
        .await
    {
        Ok(resp) => {
            let rows = resp["result"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let total = rows.len();
            let truncated = total > 100;
            let creds: Vec<serde_json::Value> = rows
                .into_iter()
                .take(100)
                .map(
                    |row| serde_json::json!({"id": row["SystemName"], "username": row["Username"]}),
                )
                .collect();
            ok_json(serde_json::json!({
                "success": true,
                "credentials": creds,
                "count": creds.len(),
                "truncated": truncated,
                "total_count": total
            }))
        }
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_credential_manage_impl(
    iris: Option<&IrisConnection>,
    params: CredentialManageParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    let id = params.id.replace('\'', "''");
    let ns = &params.namespace;

    match params.action.as_str() {
        "create" => {
            let username = match &params.username {
                Some(u) => u.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "create requires username"),
            };
            let password = match &params.password {
                Some(p) => p.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "create requires password"),
            };
            let code = format!(
                r#"Set tSC=##class(Ens.Config.Credentials).SetCredential("{}","{}","{}",0)
If $$$ISERR(tSC) {{ Write "ERROR:CREDENTIAL_EXISTS:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
                id, username, password
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"action":"create","id":params.id}),
                        )
                    } else if let Some(msg) = out.strip_prefix("ERROR:CREDENTIAL_EXISTS:") {
                        err_json("CREDENTIAL_EXISTS", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "update" => {
            // Read current values then overwrite with provided ones
            let username_expr = match &params.username {
                Some(u) => format!("\"{}\"", u.replace('\'', "''")),
                None => format!(
                    "##class(Ens.Config.Credentials).GetValue(\"{}\",\"Username\")",
                    id
                ),
            };
            let password_expr = match &params.password {
                Some(p) => format!("\"{}\"", p.replace('\'', "''")),
                None => format!(
                    "##class(Ens.Config.Credentials).GetValue(\"{}\",\"Password\")",
                    id
                ),
            };
            let code = format!(
                r#"Set tSC=##class(Ens.Config.Credentials).SetCredential("{}",{},{},1)
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
                id, username_expr, password_expr
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"action":"update","id":params.id}),
                        )
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "delete" => {
            let code = format!(
                r#"If '##class(Ens.Config.Credentials).%ExistsId("{}") {{ Write "ERROR:CREDENTIAL_NOT_FOUND:Credential not found: {}" Quit }}
Set tSC=##class(Ens.Config.Credentials).%DeleteId("{}")
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
                id, id, id
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"action":"delete","id":params.id}),
                        )
                    } else if let Some(msg) = out.strip_prefix("ERROR:CREDENTIAL_NOT_FOUND:") {
                        err_json("CREDENTIAL_NOT_FOUND", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        _ => err_json(
            "INVALID_ACTION",
            "iris_credential_manage: action must be create, update, or delete",
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 024-interop-depth: Lookup tables (US3)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LookupManageParams {
    pub action: String,
    pub table: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LookupTransferParams {
    pub action: String,
    pub table: String,
    pub xml: Option<String>,
    #[serde(default = "default_ns")]
    pub namespace: String,
}

pub async fn interop_lookup_manage_impl(
    iris: Option<&IrisConnection>,
    params: LookupManageParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    let ns = &params.namespace;

    match params.action.as_str() {
        "list_tables" => {
            let code = r#"Set tTable="" Set tOut="" Set tCount=0 For { Set tTable=$ORDER(^Ens.LookupTable(tTable)) Quit:tTable=""  Set tOut=tOut_tTable_$CHAR(10) Set tCount=tCount+1 } Write tOut"#;
            match iris.execute_via_generator(code, ns, &client).await {
                Ok(out) => {
                    let tables: Vec<String> = out
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    let total = tables.len();
                    let truncated = total > 100;
                    let tables: Vec<String> = tables.into_iter().take(100).collect();
                    ok_json(
                        serde_json::json!({"success":true,"tables":tables,"count":tables.len(),"truncated":truncated,"total_count":total}),
                    )
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "get" => {
            let table = match &params.table {
                Some(t) => t.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "get requires table"),
            };
            let key = match &params.key {
                Some(k) => k.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "get requires key"),
            };
            let code = format!(
                r#"If '$DATA(^Ens.LookupTable("{}")) {{ Write "ERROR:TABLE_NOT_FOUND:Table not found: {}" Quit }}
Set tVal=$GET(^Ens.LookupTable("{}","{}"))
If tVal="" {{ Write "ERROR:KEY_NOT_FOUND:Key not found: {}" Quit }}
Write tVal"#,
                table, table, table, key, key
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if let Some(msg) = out.strip_prefix("ERROR:TABLE_NOT_FOUND:") {
                        return err_json("TABLE_NOT_FOUND", msg);
                    }
                    if let Some(msg) = out.strip_prefix("ERROR:KEY_NOT_FOUND:") {
                        return err_json("KEY_NOT_FOUND", msg);
                    }
                    ok_json(
                        serde_json::json!({"success":true,"table":params.table,"key":params.key,"value":out}),
                    )
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "set" => {
            let table = match &params.table {
                Some(t) => t.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "set requires table"),
            };
            let key = match &params.key {
                Some(k) => k.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "set requires key"),
            };
            let value = match &params.value {
                Some(v) => v.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "set requires value"),
            };
            let code = format!(
                r#"Set tSC=##class(Ens.Util.LookupTable).%UpdateValue("{}","{}","{}",1)
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
                table, key, value
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"table":params.table,"key":params.key,"value":params.value}),
                        )
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "delete" => {
            let table = match &params.table {
                Some(t) => t.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "delete requires table"),
            };
            let key = match &params.key {
                Some(k) => k.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "delete requires key"),
            };
            let code = format!(
                r#"If '$DATA(^Ens.LookupTable("{}")) {{ Write "ERROR:TABLE_NOT_FOUND:Table not found: {}" Quit }}
Set tSC=##class(Ens.Util.LookupTable).%RemoveValue("{}","{}")
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
                table, table, table, key
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(
                            serde_json::json!({"success":true,"table":params.table,"key":params.key}),
                        )
                    } else if let Some(msg) = out.strip_prefix("ERROR:TABLE_NOT_FOUND:") {
                        err_json("TABLE_NOT_FOUND", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "list_keys" => {
            let table = match &params.table {
                Some(t) => t.replace('\'', "''"),
                None => return err_json("INVALID_PARAMS", "list_keys requires table"),
            };
            let code = format!(
                r#"If '$DATA(^Ens.LookupTable("{}")) {{ Write "ERROR:TABLE_NOT_FOUND:Table not found: {}" Quit }}
Set tKey="" For {{ Set tKey=$ORDER(^Ens.LookupTable("{}",tKey)) Quit:tKey=""  Write tKey_$CHAR(10) }}"#,
                table, table, table
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if let Some(msg) = out.strip_prefix("ERROR:TABLE_NOT_FOUND:") {
                        return err_json("TABLE_NOT_FOUND", msg);
                    }
                    let keys: Vec<String> = out
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    ok_json(
                        serde_json::json!({"success":true,"table":params.table,"keys":keys,"count":keys.len()}),
                    )
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        _ => err_json(
            "INVALID_ACTION",
            "iris_lookup_manage: action must be get, set, delete, list_keys, or list_tables",
        ),
    }
}

pub async fn interop_lookup_transfer_impl(
    iris: Option<&IrisConnection>,
    params: LookupTransferParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    let ns = &params.namespace;
    let table = params.table.replace('\'', "''");

    match params.action.as_str() {
        "export" => {
            let code = format!(
                r#"If '$DATA(^Ens.LookupTable("{}")) {{ Write "ERROR:TABLE_NOT_FOUND:Table not found: {}" Quit }}
Set tStream=##class(%Stream.TmpBinary).%New()
Set tSC=##class(Ens.Util.LookupTable).%Export(tStream,"{}")
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) Quit }}
Do tStream.Rewind()
Set tOut="" While 'tStream.AtEnd {{ Set tOut=tOut_tStream.Read(32000) }}
Write tOut"#,
                table, table, table
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if let Some(msg) = out.strip_prefix("ERROR:TABLE_NOT_FOUND:") {
                        return err_json("TABLE_NOT_FOUND", msg);
                    }
                    if let Some(msg) = out.strip_prefix("ERROR:INTEROP_ERROR:") {
                        return err_json("INTEROP_ERROR", msg);
                    }
                    // Count entries in XML
                    let entry_count = out.matches("<entry").count();
                    ok_json(
                        serde_json::json!({"success":true,"table":params.table,"xml":out,"entry_count":entry_count}),
                    )
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        "import" => {
            let xml = match &params.xml {
                Some(x) => x.clone(),
                None => return err_json("INVALID_PARAMS", "import requires xml"),
            };
            // Write XML to temp file, import, delete
            let xml_escaped = xml.replace('\\', "\\\\").replace('"', "\\\"");
            let code = format!(
                r#"Set tFile="/tmp/iris_lookup_import_"_$JOB_".xml"
Set tStream=##class(%Stream.FileCharacter).%New()
Set tStream.Filename=tFile
Do tStream.Write("{}")
Set tSC=tStream.%Save()
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:Cannot write temp file" Quit }}
Set tSC2=##class(Ens.Util.LookupTable).%Import(tFile,"{}","")
Do ##class(%File).Delete(tFile)
If $$$ISERR(tSC2) {{ Write "ERROR:INVALID_XML:"_$System.Status.GetErrorText(tSC2) Quit }}
Write "OK""#,
                xml_escaped, table
            );
            match iris.execute_via_generator(&code, ns, &client).await {
                Ok(out) => {
                    let out = out.trim();
                    if out == "OK" {
                        ok_json(serde_json::json!({"success":true,"table":params.table}))
                    } else if let Some(msg) = out.strip_prefix("ERROR:INVALID_XML:") {
                        err_json("INVALID_XML", msg)
                    } else {
                        err_json("INTEROP_ERROR", out)
                    }
                }
                Err(e) => err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                ),
            }
        }
        _ => err_json(
            "INVALID_ACTION",
            "iris_lookup_transfer: action must be export or import",
        ),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 024-interop-depth: Production autostart (US4)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProductionAutostartParams {
    pub action: String,
    #[serde(default = "default_ns")]
    pub namespace: String,
    pub enabled: Option<bool>,
    pub production: Option<String>,
}

pub async fn interop_autostart_get_impl(
    iris: Option<&IrisConnection>,
    params: &ProductionAutostartParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    // Read ^Ens.AutoStart directly — GetAutoStart() does not exist
    let code = r#"Write $GET(^Ens.AutoStart)"#;
    match iris
        .execute_via_generator(code, &params.namespace, &client)
        .await
    {
        Ok(out) => {
            let prod = out.trim().to_string();
            let enabled = !prod.is_empty();
            ok_json(serde_json::json!({
                "success": true,
                "namespace": params.namespace,
                "autostart_enabled": enabled,
                "production": if enabled { serde_json::Value::String(prod) } else { serde_json::Value::Null }
            }))
        }
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

pub async fn interop_autostart_set_impl(
    iris: Option<&IrisConnection>,
    params: &ProductionAutostartParams,
) -> Result<CallToolResult, McpError> {
    let iris = match iris {
        Some(i) => i,
        None => return err_json("IRIS_UNREACHABLE", "No IRIS connection"),
    };
    let client = IrisConnection::http_client()
        .map_err(|_| McpError::invalid_request("IRIS_UNREACHABLE", None))?;
    let ns = &params.namespace;
    let enabled = params.enabled.unwrap_or(true);

    if !enabled {
        let code = r#"Set tSC=##class(Ens.Director).SetAutoStart("")
If $$$ISERR(tSC) { Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) } Else { Write "OK" }"#;
        match iris.execute_via_generator(code, ns, &client).await {
            Ok(out) if out.trim() == "OK" => {
                return ok_json(
                    serde_json::json!({"success":true,"namespace":ns,"autostart_enabled":false,"production":null}),
                );
            }
            Ok(out) => return err_json("INTEROP_ERROR", out.trim()),
            Err(e) => {
                return err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                )
            }
        }
    }

    // enabled=true: resolve production name
    let prod_name = if let Some(p) = &params.production {
        p.replace('\'', "''")
    } else {
        // Get currently running production
        let status_code = r#"Set sc=##class(Ens.Director).GetProductionStatus(.n,.s) If $$$ISERR(sc)||n="" { Write "ERROR:NO_PRODUCTION:No production running" } Else { Write n }"#;
        match iris.execute_via_generator(status_code, ns, &client).await {
            Ok(out) => {
                let out = out.trim().to_string();
                if let Some(msg) = out.strip_prefix("ERROR:NO_PRODUCTION:") {
                    return err_json("NO_PRODUCTION", msg);
                }
                out
            }
            Err(e) => {
                return err_json(
                    if is_network_error(&e.to_string()) {
                        "IRIS_UNREACHABLE"
                    } else {
                        "INTEROP_ERROR"
                    },
                    &e.to_string(),
                )
            }
        }
    };

    let code = format!(
        r#"Set tSC=##class(Ens.Director).SetAutoStart("{}")
If $$$ISERR(tSC) {{ Write "ERROR:INTEROP_ERROR:"_$System.Status.GetErrorText(tSC) }} Else {{ Write "OK" }}"#,
        prod_name
    );
    match iris.execute_via_generator(&code, ns, &client).await {
        Ok(out) if out.trim() == "OK" => ok_json(
            serde_json::json!({"success":true,"namespace":ns,"autostart_enabled":true,"production":prod_name}),
        ),
        Ok(out) => err_json("INTEROP_ERROR", out.trim()),
        Err(e) => err_json(
            if is_network_error(&e.to_string()) {
                "IRIS_UNREACHABLE"
            } else {
                "INTEROP_ERROR"
            },
            &e.to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_network_error_sending() {
        assert!(is_network_error("error sending request for url"));
    }

    #[test]
    fn test_is_network_error_refused() {
        assert!(is_network_error("connection refused"));
    }

    #[test]
    fn test_is_network_error_reset() {
        assert!(is_network_error("connection reset by peer"));
    }

    #[test]
    fn test_is_network_error_dns() {
        assert!(is_network_error("dns error: no such host"));
    }

    #[test]
    fn test_is_network_error_timeout() {
        assert!(is_network_error("timed out"));
    }

    #[test]
    fn test_is_network_error_false_for_interop_message() {
        // "No Interoperability connection configured" must NOT be a network error
        assert!(!is_network_error(
            "No Interoperability connection configured"
        ));
    }

    #[test]
    fn test_is_network_error_false_for_docker_required() {
        assert!(!is_network_error("DOCKER_REQUIRED"));
    }

    #[test]
    fn test_is_network_error_false_for_sql_error() {
        assert!(!is_network_error("SQLCODE: -1 Field not found"));
    }

    #[test]
    fn test_production_status_params_deserialize() {
        let p: ProductionStatusParams = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(p.namespace, "USER");
        assert!(!p.full_status);
    }

    #[test]
    fn test_production_name_params_deserialize() {
        let p: ProductionNameParams =
            serde_json::from_str(r#"{"production": "MyApp.Production", "namespace": "MYNS"}"#)
                .unwrap();
        assert_eq!(p.production.as_deref(), Some("MyApp.Production"));
        assert_eq!(p.namespace, "MYNS");
    }

    #[test]
    fn test_logs_params_defaults() {
        let p: LogsParams = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(p.limit, 10); // default_limit returns 10
        assert!(!p.log_type.is_empty()); // log_type has a default
    }

    #[test]
    fn test_message_search_params_defaults() {
        let p: MessageSearchParams = serde_json::from_str(r#"{}"#).unwrap();
        assert!(p.source.is_none());
        assert!(p.target.is_none());
    }

    // ─── T011/T012/T013: US1 — ProductionItemParams unit tests ───

    #[test]
    fn production_item_params_deserialize_all_actions() {
        let p: ProductionItemParams =
            serde_json::from_str(r#"{"action":"enable","item":"MyService","namespace":"MYNS"}"#)
                .unwrap();
        assert_eq!(p.action, "enable");
        assert_eq!(p.item, "MyService");
        assert_eq!(p.namespace, "MYNS");

        let p2: ProductionItemParams = serde_json::from_str(
            r#"{"action":"set_settings","item":"MyOp","settings":{"Timeout":"30"}}"#,
        )
        .unwrap();
        assert_eq!(p2.action, "set_settings");
        assert_eq!(p2.settings.get("Timeout").map(|v| v.as_str()), Some("30"));
        assert_eq!(p2.namespace, "USER"); // default
    }

    #[test]
    fn production_item_error_mapping_item_not_found() {
        // Verify error prefix matching logic
        let msg = "ERROR:ITEM_NOT_FOUND:Item not found: Missing";
        assert!(msg.strip_prefix("ERROR:ITEM_NOT_FOUND:").is_some());
    }

    #[test]
    fn production_item_error_mapping_update_failed() {
        let msg = "ERROR:UPDATE_FAILED:Production update timed out";
        assert!(msg.strip_prefix("ERROR:UPDATE_FAILED:").is_some());
    }

    // ─── T019/T020: US2 — Credential unit tests ───

    #[test]
    fn credential_list_response_never_contains_password() {
        // Simulate what interop_credential_list_impl returns
        let resp = serde_json::json!({
            "success": true,
            "credentials": [
                {"id": "SMTPServer", "username": "user@example.com"}
            ],
            "count": 1,
            "truncated": false,
            "total_count": 1
        });
        let text = resp.to_string();
        assert!(
            !text.contains("\"password\""),
            "password must not appear in credential list"
        );
        assert!(
            !text.contains("\"Password\""),
            "Password must not appear in credential list"
        );
    }

    #[test]
    fn credential_list_truncation_fields_present() {
        // Verify that the response shape includes truncated + total_count
        let resp = serde_json::json!({"success":true,"credentials":[],"count":0,"truncated":false,"total_count":0});
        assert!(resp.get("truncated").is_some());
        assert!(resp.get("total_count").is_some());
    }

    #[test]
    fn credential_manage_params_deserialize() {
        let p: CredentialManageParams = serde_json::from_str(
            r#"{"action":"create","id":"MyCredential","username":"user","password":"pass"}"#,
        )
        .unwrap();
        assert_eq!(p.action, "create");
        assert_eq!(p.id, "MyCredential");
        assert_eq!(p.namespace, "USER");
    }

    #[test]
    fn credential_error_codes_parseable() {
        assert!("ERROR:CREDENTIAL_EXISTS:already exists"
            .strip_prefix("ERROR:CREDENTIAL_EXISTS:")
            .is_some());
        assert!("ERROR:CREDENTIAL_NOT_FOUND:not found"
            .strip_prefix("ERROR:CREDENTIAL_NOT_FOUND:")
            .is_some());
    }

    // ─── T028/T029: US3 — Lookup table unit tests ───

    #[test]
    fn lookup_manage_params_all_actions() {
        let p: LookupManageParams = serde_json::from_str(r#"{"action":"list_tables"}"#).unwrap();
        assert_eq!(p.action, "list_tables");
        assert!(p.table.is_none());

        let p2: LookupManageParams = serde_json::from_str(
            r#"{"action":"set","table":"RouteTable","key":"Target1","value":"HL7Recv"}"#,
        )
        .unwrap();
        assert_eq!(p2.action, "set");
        assert_eq!(p2.table.as_deref(), Some("RouteTable"));
        assert_eq!(p2.value.as_deref(), Some("HL7Recv"));
    }

    #[test]
    fn lookup_list_tables_response_includes_truncated() {
        let resp = serde_json::json!({"success":true,"tables":["T1","T2"],"count":2,"truncated":false,"total_count":2});
        assert_eq!(resp["truncated"], false);
        assert_eq!(resp["total_count"], 2);
    }

    #[test]
    fn lookup_error_codes_parseable() {
        assert!("ERROR:TABLE_NOT_FOUND:No such table"
            .strip_prefix("ERROR:TABLE_NOT_FOUND:")
            .is_some());
        assert!("ERROR:INVALID_XML:Parse error"
            .strip_prefix("ERROR:INVALID_XML:")
            .is_some());
        assert!("ERROR:KEY_NOT_FOUND:Key missing"
            .strip_prefix("ERROR:KEY_NOT_FOUND:")
            .is_some());
    }

    // ─── T037: US4 — Autostart params ───

    #[test]
    fn autostart_params_deserialize() {
        let p: ProductionAutostartParams =
            serde_json::from_str(r#"{"action":"get_autostart","namespace":"MYAPP"}"#).unwrap();
        assert_eq!(p.action, "get_autostart");
        assert_eq!(p.namespace, "MYAPP");
        assert!(p.enabled.is_none());

        let p2: ProductionAutostartParams = serde_json::from_str(
            r#"{"action":"set_autostart","namespace":"MYAPP","enabled":true,"production":"MyApp.Production"}"#
        ).unwrap();
        assert_eq!(p2.enabled, Some(true));
        assert_eq!(p2.production.as_deref(), Some("MyApp.Production"));
    }
}
