//! IRIS instance discovery cascade.
//!
//! Order of priority (highest to lowest):
//! 1. Explicit IrisConnection passed directly
//! 2. Env vars (IRIS_HOST + IRIS_WEB_PORT)
//! 3. Localhost port scan (100ms timeout, parallel)
//! 4. Docker containers via bollard
//! 5. VS Code settings.json objectscript.conn
//!
//! Each step fails silently and falls through to the next.

use anyhow::Result;
use std::time::Duration;
use crate::iris::connection::{IrisConnection, DiscoverySource};

/// The ports we scan on localhost for IRIS web servers.
const IRIS_WEB_PORTS: &[u16] = &[52773, 41773, 51773, 8080];

/// Probe a single host:port via Atelier REST. Returns Some(conn) if IRIS found.
pub async fn probe_atelier(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    namespace: &str,
    timeout_ms: u64,
) -> Option<IrisConnection> {
    let base_url = format!("http://{}:{}", host, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let resp = client
        .get(format!("{}/api/atelier/", base_url))
        .basic_auth(username, Some(password))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;

    // Fingerprint: result.content[0].version must contain "IRIS"
    let version = body["result"]["content"][0]["version"]
        .as_str()
        .filter(|v| v.to_uppercase().contains("IRIS"))
        .map(|v| v.to_string())?;

    let mut conn = IrisConnection::new(
        base_url,
        namespace,
        username,
        password,
        DiscoverySource::LocalhostScan { port },
    );
    conn.version = Some(version);
    Some(conn)
}

/// Full discovery cascade. Returns Ok(Some(conn)) if IRIS found, Ok(None) if not.
pub async fn discover_iris(explicit: Option<IrisConnection>) -> Result<Option<IrisConnection>> {
    // 1. Explicit wins immediately
    if let Some(conn) = explicit {
        return Ok(Some(conn));
    }

    // 2. Env vars
    if let Ok(host) = std::env::var("IRIS_HOST") {
        let port = std::env::var("IRIS_WEB_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(52773);
        let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
        let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());
        let namespace = std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string());

        if let Some(mut conn) = probe_atelier(&host, port, &username, &password, &namespace, 5000).await {
            conn.source = DiscoverySource::EnvVar;
            return Ok(Some(conn));
        }
    }

    // 3. Localhost scan (parallel, 100ms each)
    let scan_tasks: Vec<_> = IRIS_WEB_PORTS.iter().map(|&port| {
        tokio::spawn(async move {
            probe_atelier("localhost", port, "_SYSTEM", "SYS", "USER", 100).await
        })
    }).collect();

    for task in scan_tasks {
        if let Ok(Some(conn)) = task.await {
            return Ok(Some(conn));
        }
    }

    // 4. Docker scan via bollard
    if let Some(conn) = discover_via_docker().await {
        return Ok(Some(conn));
    }

    // 5. VS Code settings.json
    if let Some(conn) = discover_via_vscode_settings().await {
        return Ok(Some(conn));
    }

    Ok(None)
}

/// Score a container name against a workspace basename (spec-025 scoring rules).
pub fn score_container_name(container_name: &str, workspace_basename: &str) -> u32 {
    if workspace_basename.is_empty() {
        return 0;
    }
    let cn = container_name.to_lowercase();
    let wb = workspace_basename.to_lowercase();

    let base = if cn == wb {
        100
    } else if cn.starts_with(&wb) {
        80
    } else if cn.contains(&wb) {
        60
    } else {
        0
    };

    if base == 0 {
        return 0;
    }

    let suffix_bonus = if cn.ends_with("-iris") || cn.ends_with("_iris") { 10 } else { 0 }
        + if cn.ends_with("-test") || cn.ends_with("_test") { 5 } else { 0 };

    base + suffix_bonus
}

async fn discover_via_docker() -> Option<IrisConnection> {
    use bollard::Docker;
    use bollard::container::ListContainersOptions;

    let workspace_basename = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();

    let docker = Docker::connect_with_defaults().ok()?;
    let containers = docker.list_containers(
        Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        })
    ).await.ok()?;

    let mut candidates: Vec<(u32, String, u16, Option<u16>)> = Vec::new();

    for container in containers {
        let image = container.image.as_deref().unwrap_or("");
        if !image.contains("intersystems") && !image.contains("iris") {
            continue;
        }

        let container_name = container.names.clone()
            .and_then(|n| n.into_iter().next())
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();

        let mut port_web: Option<u16> = None;
        let mut port_superserver: Option<u16> = None;

        if let Some(ports) = container.ports {
            for port in &ports {
                if port.private_port == 52773 {
                    port_web = port.public_port.map(|p| p as u16);
                }
                if port.private_port == 1972 {
                    port_superserver = port.public_port.map(|p| p as u16);
                }
            }
        }

        if let Some(web_port) = port_web {
            let score = score_container_name(&container_name, &workspace_basename);
            candidates.push((score, container_name, web_port, port_superserver));
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));

    for (_score, container_name, web_port, port_ss) in candidates {
        if let Some(mut conn) = probe_atelier(
            "localhost", web_port, "_SYSTEM", "SYS", "USER", 500
        ).await {
            conn.source = DiscoverySource::Docker { container_name };
            conn.port_superserver = port_ss;
            return Some(conn);
        }
    }
    None
}

/// Attempt to find IRIS connection from VS Code settings.json in common locations.
async fn discover_via_vscode_settings() -> Option<IrisConnection> {
    let candidates = [
        std::env::current_dir().ok()?.join(".vscode/settings.json"),
    ];

    for path in &candidates {
        if !path.exists() { continue; }
        if let Ok(settings) = crate::iris::vscode_config::parse_vscode_settings(path) {
            if let Some(conn) = settings.to_iris_connection().await {
                return Some(conn);
            }
        }
    }
    None
}
