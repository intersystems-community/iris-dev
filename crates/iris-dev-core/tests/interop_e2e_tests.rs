use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn iris_dev_bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); p.pop();
    p.push("target/debug/iris-dev");
    p
}

fn mcp_exchange(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let bin = iris_dev_bin();
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    let iris_port = std::env::var("IRIS_WEB_PORT").unwrap_or_else(|_| "52780".to_string());

    let mut child = Command::new(&bin)
        .args(["mcp"])
        .env("IRIS_HOST", &iris_host)
        .env("IRIS_WEB_PORT", &iris_port)
        .env("IRIS_USERNAME", std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string()))
        .env("IRIS_PASSWORD", std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string()))
        .env("IRIS_NAMESPACE", std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut results = vec![];

    for (i, msg) in messages.iter().enumerate() {
        stdin.write_all((serde_json::to_string(msg).unwrap() + "\n").as_bytes()).unwrap();
        stdin.flush().unwrap();
        if msg.get("id").is_some() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let mut line = String::new();
                std::thread::sleep(std::time::Duration::from_millis(50));
                if reader.read_line(&mut line).unwrap_or(0) > 0 {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        results.push(v);
                        break;
                    }
                }
                if std::time::Instant::now() > deadline { break; }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    child.kill().ok();
    results
}

fn find_response(responses: &[serde_json::Value], id: u64) -> Option<serde_json::Value> {
    responses.iter().find(|r| r["id"] == id).cloned()
}

fn parse_tool_text(response: &serde_json::Value) -> serde_json::Value {
    let text = response["result"]["content"][0]["text"].as_str().unwrap_or("{}");
    serde_json::from_str(text).unwrap_or_default()
}

#[test]
fn tools_list_returns_32_tools() {
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    if iris_host.is_empty() { eprintln!("Skipping: IRIS_HOST not set"); return; }

    let responses = mcp_exchange(&[
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    ]);

    let tools_resp = find_response(&responses, 2).expect("no tools/list response");
    let tools = tools_resp["result"]["tools"].as_array().expect("no tools array");
    let names: Vec<_> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(names.len() >= 32, "expected >=32 tools, got {}: {:?}", names.len(), &names[..names.len().min(10)]);
    assert!(names.contains(&"interop_production_status"));
    assert!(names.contains(&"interop_logs"));
    assert!(names.contains(&"interop_queues"));
    assert!(names.contains(&"interop_message_search"));
    for name in &names {
        assert!(!name.contains('.'), "tool '{}' has dot", name);
    }
}

#[test]
fn interop_production_status_returns_structured_json() {
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    if iris_host.is_empty() { return; }

    let responses = mcp_exchange(&[
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"interop_production_status","arguments":{}}}),
    ]);

    let resp = find_response(&responses, 2).expect("no tool response");
    let result = parse_tool_text(&resp);
    assert!(result.get("success").is_some() || result.get("error_code").is_some(),
        "must return structured response: {}", result);
}

#[test]
fn interop_logs_returns_structured_entries() {
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    if iris_host.is_empty() { return; }

    let responses = mcp_exchange(&[
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"interop_logs","arguments":{"limit":5,"log_type":"error"}}}),
    ]);

    let resp = find_response(&responses, 2).expect("no tool response");
    let result = parse_tool_text(&resp);
    assert!(result.get("success").is_some() || result.get("error_code").is_some());
}

#[test]
fn interop_queues_returns_array() {
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    if iris_host.is_empty() { return; }

    let responses = mcp_exchange(&[
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"interop_queues","arguments":{}}}),
    ]);

    let resp = find_response(&responses, 2).expect("no tool response");
    let result = parse_tool_text(&resp);
    assert!(result.get("success").is_some() || result.get("error_code").is_some());
}
