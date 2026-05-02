# Contract: Discovery API Changes

## `discover_via_docker_named()` — new signature

```rust
// crates/iris-dev-core/src/iris/discovery.rs

// Before
async fn discover_via_docker_named(target: &str) -> Option<IrisConnection>

// After
async fn discover_via_docker_named(target: &str) -> DiscoveryResult
```

### Returns
- `DiscoveryResult::Connected(IrisConnection)` — probe succeeded, connection ready
- `DiscoveryResult::NotFound` — no container with matching name in Docker API
- `DiscoveryResult::FoundUnhealthy(FailureMode)` — container found, probe failed

### Does NOT emit log messages itself
`discover_via_docker_named` is a pure function — it returns a structured result. The caller
(`discover_iris`) is responsible for emitting the mode-specific log message.

---

## `discover_iris()` — new signature

```rust
// Before
pub async fn discover_iris(explicit: Option<IrisConnection>) -> Result<Option<IrisConnection>>

// After
pub async fn discover_iris(explicit: Option<IrisConnection>) -> IrisDiscovery
```

Note: error cases (e.g. building the HTTP client) that currently return `Err` are now
logged as warnings and return `IrisDiscovery::NotFound` — the function is infallible from
the caller's perspective.

### Caller migration

**mcp.rs** — before:
```rust
let conn = match discover_iris(explicit).await {
    Ok(c) => c,
    Err(e) => { tracing::warn!("IRIS discovery error: {}", e); None }
};
if conn.is_none() {
    tracing::warn!("No IRIS connection — tools return IRIS_UNREACHABLE");
}
let _ = iris_tx.send(conn);
```

**mcp.rs** — after:
```rust
let conn = match discover_iris(explicit).await {
    IrisDiscovery::Found(c) => Some(c),
    IrisDiscovery::NotFound => {
        tracing::warn!("No IRIS connection — tools return IRIS_UNREACHABLE");
        None
    }
    IrisDiscovery::Explained => None,  // message already emitted — silent
};
let _ = iris_tx.send(conn);
```

**compile.rs** — before:
```rust
let iris = discover_iris(explicit).await?.context("No IRIS connection found ...")?;
```

**compile.rs** — after:
```rust
let iris = match discover_iris(explicit).await {
    IrisDiscovery::Found(c) => c,
    IrisDiscovery::NotFound => {
        anyhow::bail!("No IRIS connection found — set IRIS_HOST or run iris-dev mcp for auto-discovery");
    }
    IrisDiscovery::Explained => std::process::exit(1),
};
```

---

## `probe_atelier_with_client()` — behavior change

The 401 WARN message is updated to include the container name when called from the docker
named discovery path. This requires threading the container name through.

```rust
// New helper signature (internal)
async fn probe_atelier_for_container(
    client: &reqwest::Client,
    container_name: &str,
    port: u16,
    username: &str,
    password: &str,
    namespace: &str,
) -> DiscoveryResult
```

This replaces the direct call to `probe_atelier()` within `discover_via_docker_named()`,
allowing the 401 message to include the container name and suppressing the second generic WARN.

---

## Localhost scan — credential change

```rust
// Before (line 176 in discovery.rs)
probe_atelier_with_client(&client, "localhost", port, "_SYSTEM", "SYS", "USER")

// After
let username = std::env::var("IRIS_USERNAME").unwrap_or_else(|_| "_SYSTEM".to_string());
let password = std::env::var("IRIS_PASSWORD").unwrap_or_else(|_| "SYS".to_string());
let namespace = std::env::var("IRIS_NAMESPACE").unwrap_or_else(|_| "USER".to_string());
probe_atelier_with_client(&client, "localhost", port, &username, &password, &namespace)
```
