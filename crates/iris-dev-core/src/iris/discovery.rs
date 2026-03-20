use anyhow::Result;
use crate::iris::connection::{IrisConnection, DiscoverySource};

/// Attempt to probe an IRIS instance at a given host:port via Atelier REST.
/// Returns Some(IrisConnection) if the probe succeeds, None otherwise.
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
        .timeout(std::time::Duration::from_millis(timeout_ms))
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
    let version = body["result"]["content"][0]["version"]
        .as_str()
        .filter(|v| v.to_lowercase().contains("iris"))
        .map(|v| v.to_string())?;

    let mut conn = IrisConnection::new(
        base_url, namespace, username, password,
        DiscoverySource::LocalhostScan { port },
    );
    conn.version = Some(version);
    Some(conn)
}

/// Full discovery cascade:
/// 1. Localhost port scan (100ms timeout)
/// 2. Docker containers via bollard
/// 3. VS Code settings.json
/// 4. Env vars
/// 5. Explicit config (passed in)
pub async fn discover_iris(
    explicit: Option<IrisConnection>,
) -> Result<Option<IrisConnection>> {
    // Priority 0: explicit config wins immediately
    if let Some(conn) = explicit {
        return Ok(Some(conn));
    }

    // Priority 1: env vars
    if let (Ok(host), Ok(port)) = (
        std::env::var("IRIS_HOST"),
        std::env::var("IRIS_WEB_PORT").map(|p| p.parse::<u16>().unwrap_or(52773)),
    ) {
        let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
        let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());
        let namespace = std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string());
        if let Some(conn) = probe_atelier(&host, port, &username, &password, &namespace, 5000).await {
            return Ok(Some(conn));
        }
    }

    // Priority 2: localhost scan
    let iris_ports: &[u16] = &[52773, 41773, 51773, 8080];
    let scan_futures: Vec<_> = iris_ports.iter().map(|&port| {
        probe_atelier("localhost", port, "_SYSTEM", "SYS", "USER", 100)
    }).collect();

    for fut in scan_futures {
        if let Some(conn) = fut.await {
            return Ok(Some(conn));
        }
    }

    // Priority 3: Docker (TODO: implement bollard scan)
    // Priority 4: VS Code settings.json (TODO: implement)

    Ok(None)
}
