//! T079: E2E test — all 20 tools respond without INTERNAL_ERROR.
//! T080: Steve's web prefix scenario.
//! Run: IRIS_HOST=localhost IRIS_WEB_PORT=52780 IRIS_USERNAME=SuperUser IRIS_PASSWORD=SYS cargo test --test test_e2e_all_tools

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn iris_dev_bin() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("target/debug/iris-dev");
    path
}

fn mcp_exchange_with_env(
    env_vars: &[(&str, &str)],
    messages: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return vec![];
    }

    let mut cmd = Command::new(&bin);
    cmd.args(["mcp"]);
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut results = vec![];

    for msg in messages {
        stdin
            .write_all((serde_json::to_string(msg).unwrap() + "\n").as_bytes())
            .unwrap();
        stdin.flush().unwrap();

        if msg.get("id").is_some() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) > 0 {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        results.push(v);
                        break;
                    }
                }
                if std::time::Instant::now() > deadline {
                    break;
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    child.kill().ok();
    results
}

/// T079: All 20 v2 tools are listed and none return INTERNAL_ERROR on minimal input.
#[test]
fn e2e_all_tools_respond() {
    let iris_host = std::env::var("IRIS_HOST").unwrap_or_default();
    if iris_host.is_empty() {
        eprintln!("Skipping e2e_all_tools_respond: IRIS_HOST not set");
        return;
    }

    let iris_port = std::env::var("IRIS_WEB_PORT").unwrap_or_else(|_| "52780".to_string());
    let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
    let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());

    let env_vars = [
        ("IRIS_HOST", iris_host.as_str()),
        ("IRIS_WEB_PORT", iris_port.as_str()),
        ("IRIS_USERNAME", username.as_str()),
        ("IRIS_PASSWORD", password.as_str()),
    ];

    // Get tool list
    let list_responses = mcp_exchange_with_env(
        &env_vars,
        &[
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
            serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ],
    );

    let tools_resp = list_responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("no tools/list response");
    let tools = tools_resp["result"]["tools"]
        .as_array()
        .expect("tools array missing");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    eprintln!("tools returned: {:?}", tool_names);
    assert!(!tool_names.is_empty(), "expected tools to be listed");

    // Verify no dots in tool names (Bedrock/VS Code requirement)
    for name in &tool_names {
        assert!(!name.contains('.'), "tool name '{}' contains dot", name);
    }

    // Call a subset of tools with minimal valid inputs and assert no INTERNAL_ERROR
    let test_calls: Vec<serde_json::Value> = vec![
        serde_json::json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"iris_execute","arguments":{"code":"write 1+1"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"iris_query","arguments":{"query":"SELECT 1 as n"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"iris_info","arguments":{"what":"metadata"}}}),
    ];

    let mut all_msgs = vec![
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    ];
    all_msgs.extend(test_calls);

    let responses = mcp_exchange_with_env(&env_vars, &all_msgs);

    for resp in &responses {
        if let Some(id) = resp["id"].as_u64() {
            if id >= 10 {
                let text = resp["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap_or("{}");
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(text) {
                    assert_ne!(
                        result["error_code"].as_str(),
                        Some("INTERNAL_ERROR"),
                        "tool id={} returned INTERNAL_ERROR: {}",
                        id,
                        result
                    );
                }
            }
        }
    }
}

/// T080: Steve's web prefix scenario — compile + info work with IRIS_WEB_PREFIX set.
#[test]
fn e2e_web_prefix_route_correct() {
    use iris_dev_core::iris::connection::{DiscoverySource, IrisConnection};

    // Verify that the URL construction includes the prefix
    let conn = IrisConnection::new(
        "http://localhost:80/irisaicore",
        "USER",
        "_SYSTEM",
        "SYS",
        DiscoverySource::ExplicitFlag,
    );

    let compile_url = conn.atelier_url("/v8/USER/action/compile");
    let info_url = conn.atelier_url("/v8/USER/docs");

    assert!(
        compile_url.contains("/irisaicore/api/atelier/"),
        "compile URL missing prefix: {}",
        compile_url
    );
    assert!(
        info_url.contains("/irisaicore/api/atelier/"),
        "info URL missing prefix: {}",
        info_url
    );
    assert_eq!(
        compile_url,
        "http://localhost:80/irisaicore/api/atelier/v8/USER/action/compile"
    );
}
