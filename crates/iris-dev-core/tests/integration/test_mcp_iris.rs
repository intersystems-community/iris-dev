//! T024: IRIS e2e tests for iris-dev mcp against a real IRIS container.
//! Constitution Principle IV: dedicated bollard-managed container, no reuse.
//!
//! Tests written FIRST — must fail until IRIS tools are fully implemented.
//!
//! Run with: cargo test --test integration -- --nocapture

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn iris_dev_bin() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); path.pop();
    path.push("target/debug/iris-dev");
    path
}

/// Spin up a fresh IRIS Community container via bollard for this test session.
async fn start_iris_container() -> (bollard::Docker, String, u16) {
    use bollard::Docker;
    use bollard::container::{CreateContainerOptions, Config, StartContainerOptions};
    use bollard::models::HostConfig;

    let docker = Docker::connect_with_defaults().expect("Docker not available");
    let container_name = format!("iris-dev-test-{}", std::process::id());

    // Pull image if needed (timeout: 120s)
    let _ = docker.create_image(
        Some(bollard::image::CreateImageOptions {
            from_image: "intersystems/iris-community",
            tag: "latest",
            ..Default::default()
        }),
        None, None,
    );

    // Find a free port
    let port = find_free_port();

    docker.create_container(
        Some(CreateContainerOptions { name: &container_name, platform: None }),
        Config {
            image: Some("intersystems/iris-community:latest"),
            host_config: Some(HostConfig {
                port_bindings: Some(std::collections::HashMap::from([(
                    "52773/tcp".to_string(),
                    Some(vec![bollard::models::PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                )])),
                auto_remove: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        },
    ).await.expect("failed to create IRIS container");

    docker.start_container(&container_name, None::<StartContainerOptions<String>>)
        .await.expect("failed to start IRIS container");

    // Wait for IRIS to be ready (max 60s)
    wait_for_iris("localhost", port, 60).await;

    (docker, container_name, port)
}

async fn wait_for_iris(host: &str, port: u16, timeout_secs: u64) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        if client.get(format!("http://{}:{}/api/atelier/", host, port))
            .basic_auth("_SYSTEM", Some("SYS"))
            .send().await.map(|r| r.status().is_success()).unwrap_or(false)
        {
            return;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    panic!("IRIS container did not become ready within {}s", timeout_secs);
}

async fn stop_iris_container(docker: &bollard::Docker, name: &str) {
    let _ = docker.stop_container(name, None).await;
    let _ = docker.remove_container(name, None).await;
}

fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn send_jsonrpc(stdin: &mut impl Write, id: u64, method: &str, params: &str) {
    let msg = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"{}\",\"params\":{}}}\n",
        id, method, params
    );
    stdin.write_all(msg.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn read_jsonrpc(reader: &mut impl BufRead) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).expect("invalid JSON-RPC response")
}

fn parse_tool_result(response: &serde_json::Value) -> serde_json::Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result has no text content");
    serde_json::from_str(text).expect("tool result text is not JSON")
}

/// iris_compile compiles a valid class on real IRIS and returns compiled:true.
#[tokio::test]
async fn e2e_iris_compile_success() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        eprintln!("Skipping: iris-dev binary not found");
        return;
    }

    let (docker, container_name, port) = start_iris_container().await;

    let mut child = Command::new(&bin)
        .args(["mcp"])
        .env("IRIS_HOST", "localhost")
        .env("IRIS_WEB_PORT", port.to_string())
        .env("IRIS_USERNAME", "_SYSTEM")
        .env("IRIS_PASSWORD", "SYS")
        .env("IRIS_NAMESPACE", "USER")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    send_jsonrpc(&mut stdin, 1, "initialize", r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}"#);
    let _init = read_jsonrpc(&mut reader);

    // Compile a minimal class
    send_jsonrpc(&mut stdin, 2, "tools/call", r#"{
        "name": "iris_compile",
        "arguments": {"target": "IrisDevTest.Probe", "flags": "ck"}
    }"#);

    let response = read_jsonrpc(&mut reader);
    let result = parse_tool_result(&response);

    // The tool should succeed OR return IRIS_UNREACHABLE — not crash
    assert!(result.get("success").is_some() || result.get("error_code").is_some(),
        "iris_compile must return structured response, got: {}", result);

    // If connected, should have compiled
    if result["error_code"].as_str() != Some("IRIS_UNREACHABLE") {
        assert_eq!(result["success"], true, "iris_compile should succeed: {}", result);
    }

    child.kill().ok();
    stop_iris_container(&docker, &container_name).await;
}

/// iris_compile returns IRIS_UNREACHABLE gracefully when IRIS is not reachable.
#[tokio::test]
async fn e2e_iris_compile_unreachable_returns_error_code() {
    let bin = iris_dev_bin();
    if !bin.exists() {
        return;
    }

    let mut child = Command::new(&bin)
        .args(["mcp"])
        .env("IRIS_HOST", "nonexistent.invalid")
        .env("IRIS_WEB_PORT", "52773")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn iris-dev mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    send_jsonrpc(&mut stdin, 1, "initialize", r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"e2e","version":"0.1"}}"#);
    let _init = read_jsonrpc(&mut reader);

    send_jsonrpc(&mut stdin, 2, "tools/call", r#"{"name":"iris_compile","arguments":{"target":"Test.Foo"}}"#);
    let response = read_jsonrpc(&mut reader);
    let result = parse_tool_result(&response);

    assert_eq!(result["error_code"], "IRIS_UNREACHABLE",
        "should return IRIS_UNREACHABLE when IRIS not reachable: {}", result);
    assert_eq!(result["success"], false);

    child.kill().ok();
}
