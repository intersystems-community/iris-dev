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

use crate::iris::connection::{DiscoverySource, IrisConnection};
use anyhow::Result;
use std::time::Duration;

/// The ports we scan on localhost for IRIS web servers.
const IRIS_WEB_PORTS: &[u16] = &[52773, 41773, 51773, 8080];

/// Inner probe using a pre-built HTTP client. Avoids creating a new client per probe (Bug 24).
async fn probe_atelier_with_client(
    client: &reqwest::Client,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    namespace: &str,
) -> Option<IrisConnection> {
    let base_url = format!("http://{}:{}", host, port);
    let resp = client
        .get(format!("{}/api/atelier/", base_url))
        .basic_auth(username, Some(password))
        .send()
        .await
        .ok()?;

    if resp.status().as_u16() == 401 {
        // #21: iris-community containers started without IRIS_PASSWORD have OS auth only.
        // Basic auth is rejected. Log a hint so the user knows what to do.
        tracing::warn!(
            "IRIS at {}:{} returned 401 — container may need IRIS_PASSWORD. \
             Restart with: docker run -e IRIS_PASSWORD=SYS ...",
            host,
            port
        );
        return None;
    }
    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    let content = &body["result"]["content"];

    let version = content["version"]
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
    conn.atelier_version = match content["api"].as_u64() {
        Some(v) if v >= 8 => crate::iris::connection::AtelierVersion::V8,
        Some(v) if v >= 2 => crate::iris::connection::AtelierVersion::V2,
        _ => crate::iris::connection::AtelierVersion::V1,
    };
    Some(conn)
}

/// Probe a single host:port via Atelier REST. Returns Some(conn) if IRIS found.
pub async fn probe_atelier(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    namespace: &str,
    timeout_ms: u64,
) -> Option<IrisConnection> {
    // Bug 24: for the public API we still create a client here, but internal callers
    // (localhost scan, docker probe) reuse a shared client via probe_atelier_with_client.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;
    probe_atelier_with_client(&client, host, port, username, password, namespace).await
}

/// Full discovery cascade. Returns Ok(Some(conn)) if IRIS found, Ok(None) if not.
pub async fn discover_iris(explicit: Option<IrisConnection>) -> Result<Option<IrisConnection>> {
    // 1. Explicit wins immediately — but probe for version + Atelier API level first
    if let Some(mut conn) = explicit {
        conn.probe().await;
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
        let scheme = std::env::var("IRIS_SCHEME")
            .ok()
            .map(|s| s.trim_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http".to_string());
        let prefix = std::env::var("IRIS_WEB_PREFIX")
            .ok()
            .map(|p| p.trim_matches('/').to_string())
            .filter(|p| !p.is_empty());

        // When scheme or prefix is set, build base_url directly — probe_atelier
        // hardcodes http:// and doesn't support prefixes.
        if scheme != "http" || prefix.is_some() {
            let base_url = match &prefix {
                Some(p) => format!("{}://{}:{}/{}", scheme, host, port, p),
                None => format!("{}://{}:{}", scheme, host, port),
            };
            let mut conn = IrisConnection::new(
                base_url,
                namespace,
                username,
                password,
                DiscoverySource::EnvVar,
            );
            conn.probe().await;
            return Ok(Some(conn));
        }

        if let Some(mut conn) =
            probe_atelier(&host, port, &username, &password, &namespace, 5000).await
        {
            conn.source = DiscoverySource::EnvVar;
            return Ok(Some(conn));
        }
    }

    // 3. IRIS_CONTAINER: resolve the named container's web port via Docker.
    // workspace_config sets this when container = "..." in .iris-dev.toml.
    // Must run before the generic localhost scan or another container on port 52773 wins.
    if let Ok(container_name) = std::env::var("IRIS_CONTAINER") {
        if !container_name.is_empty() {
            if let Some(conn) = discover_via_docker_named(&container_name).await {
                return Ok(Some(conn));
            }
            tracing::warn!(
                "IRIS_CONTAINER={} not found or not reachable via Docker",
                container_name
            );
        }
    }

    // 4. Localhost scan (parallel, 100ms each).
    // Bug 24: share a single client across all port probes.
    let scan_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(100))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_default();
    let scan_client = std::sync::Arc::new(scan_client);

    let scan_tasks: Vec<_> = IRIS_WEB_PORTS
        .iter()
        .map(|&port| {
            let client = scan_client.clone();
            tokio::spawn(async move {
                probe_atelier_with_client(&client, "localhost", port, "_SYSTEM", "SYS", "USER")
                    .await
            })
        })
        .collect();

    for task in scan_tasks {
        if let Ok(Some(conn)) = task.await {
            return Ok(Some(conn));
        }
    }

    // 5. Docker scan via bollard
    if let Some(conn) = discover_via_docker().await {
        return Ok(Some(conn));
    }

    // 6. VS Code settings.json
    if let Some(conn) = discover_via_vscode_settings().await {
        return Ok(Some(conn));
    }

    Ok(None)
}

/// Score a container name against a workspace basename (spec-025 scoring rules).
/// Returns a higher score for closer name matches to the workspace.
pub fn score_container_name(container_name: &str, workspace_basename: &str) -> u32 {
    if workspace_basename.is_empty() {
        return 0;
    }
    let cn = container_name.to_lowercase().replace('-', "_");
    let wb = workspace_basename.to_lowercase().replace('-', "_");

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

    let suffix_bonus = if cn.ends_with("-iris") || cn.ends_with("_iris") {
        10
    } else {
        0
    } + if cn.ends_with("-test") || cn.ends_with("_test") {
        5
    } else {
        0
    };

    base + suffix_bonus
}

/// Resolve a specific named container to its web port and probe it.
/// Used when IRIS_CONTAINER env var is set (e.g. from .iris-dev.toml).
async fn discover_via_docker_named(target: &str) -> Option<IrisConnection> {
    use bollard::container::ListContainersOptions;
    use bollard::Docker;

    let docker = Docker::connect_with_defaults().ok()?;
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .ok()?;

    let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
    let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());
    let namespace = std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string());

    for container in containers {
        let name = container
            .names
            .and_then(|n| n.into_iter().next())
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();

        if name != target {
            continue;
        }

        let mut port_web: Option<u16> = None;
        let mut port_ss: Option<u16> = None;
        for port in container.ports.unwrap_or_default() {
            if port.private_port == 52773 {
                port_web = port.public_port;
            }
            if port.private_port == 1972 {
                port_ss = port.public_port;
            }
        }

        if let Some(web_port) = port_web {
            if let Some(mut conn) = probe_atelier(
                "localhost",
                web_port,
                &username,
                &password,
                &namespace,
                2000,
            )
            .await
            {
                conn.source = DiscoverySource::Docker {
                    container_name: name,
                };
                conn.port_superserver = port_ss;
                return Some(conn);
            }
        }
    }
    None
}

async fn discover_via_docker() -> Option<IrisConnection> {
    use bollard::container::ListContainersOptions;
    use bollard::Docker;

    let workspace_basename = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();

    let docker = Docker::connect_with_defaults().ok()?;
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .ok()?;

    let mut candidates: Vec<(u32, String, u16, Option<u16>)> = Vec::new();

    for container in containers {
        let image = container.image.as_deref().unwrap_or("");
        if !image.contains("intersystems") && !image.contains("iris") {
            continue;
        }

        let container_name = container
            .names
            .clone()
            .and_then(|n| n.into_iter().next())
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();

        let mut port_web: Option<u16> = None;
        let mut port_superserver: Option<u16> = None;

        if let Some(ports) = container.ports {
            for port in &ports {
                if port.private_port == 52773 {
                    port_web = port.public_port;
                }
                if port.private_port == 1972 {
                    port_superserver = port.public_port;
                }
            }
        }

        if let Some(web_port) = port_web {
            let score = score_container_name(&container_name, &workspace_basename);
            candidates.push((score, container_name, web_port, port_superserver));
        }
    }

    candidates.sort_by_key(|b| std::cmp::Reverse(b.0));

    // Bug 12: use IRIS_USERNAME/IRIS_PASSWORD env vars instead of hardcoded credentials.
    let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
    let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());

    // Bug 24: create a single shared HTTP client for all docker probes.
    let probe_client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    for (_score, container_name, web_port, port_ss) in candidates {
        if let Some(mut conn) = probe_atelier_with_client(
            &probe_client,
            "localhost",
            web_port,
            &username,
            &password,
            "USER",
        )
        .await
        {
            conn.source = DiscoverySource::Docker { container_name };
            conn.port_superserver = port_ss;
            return Some(conn);
        }
    }
    None
}

/// Attempt to find IRIS connection from VS Code settings.json in common locations.
async fn discover_via_vscode_settings() -> Option<IrisConnection> {
    let candidates = [std::env::current_dir().ok()?.join(".vscode/settings.json")];

    for path in &candidates {
        if !path.exists() {
            continue;
        }
        if let Ok(settings) = crate::iris::vscode_config::parse_vscode_settings(path) {
            if let Some(conn) = settings.to_iris_connection().await {
                return Some(conn);
            }
        }
    }
    None
}
