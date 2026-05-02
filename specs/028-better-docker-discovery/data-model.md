# Data Model: Better Docker Discovery Error Messages

## Entities

### FailureMode

Encodes the specific reason a named container probe failed. Returned inside `DiscoveryResult::FoundUnhealthy`.

| Variant | Fields | Meaning |
|---------|--------|---------|
| `PortNotMapped` | — | Container found; port 52773 has no host mapping |
| `AtelierNotResponding` | `port: u16` | Container + port found; HTTP probe got no response (connection refused, timeout, empty reply) |
| `AtelierHttpError` | `port: u16, status: u16` | Container + port found; HTTP probe returned non-200, non-401 status |
| `AtelierAuth401` | `port: u16` | Container + port found; HTTP probe returned 401 (already logged by `probe_atelier_with_client`) |

```rust
pub enum FailureMode {
    PortNotMapped,
    AtelierNotResponding { port: u16 },
    AtelierHttpError { port: u16, status: u16 },
    AtelierAuth401 { port: u16 },
}
```

---

### DiscoveryResult

Return type of `discover_via_docker_named()`. Replaces `Option<IrisConnection>`.

| Variant | Payload | Cascade behavior |
|---------|---------|-----------------|
| `Connected(IrisConnection)` | Fully probed connection | Return immediately to caller |
| `NotFound` | — | Caller may continue cascade |
| `FoundUnhealthy(FailureMode)` | Why it failed | Caller MUST stop cascade, emit mode-specific message, return `IrisDiscovery::Explained` |

```rust
pub enum DiscoveryResult {
    Connected(IrisConnection),
    NotFound,
    FoundUnhealthy(FailureMode),
}
```

---

### IrisDiscovery

Return type of `discover_iris()`. Replaces `Result<Option<IrisConnection>>`.

| Variant | Meaning | Caller behavior |
|---------|---------|----------------|
| `Found(IrisConnection)` | Connection established | Proceed normally |
| `NotFound` | No IRIS found anywhere in cascade | Emit "No IRIS connection" warn; tools return `IRIS_UNREACHABLE` |
| `Explained` | Discovery emitted a specific actionable message | Silently stop — emit nothing further |

```rust
pub enum IrisDiscovery {
    Found(IrisConnection),
    NotFound,
    Explained,
}
```

**Caller contract**:
- `mcp.rs`: `Explained` → proceed with `conn = None`, skip "No IRIS connection" warn
- `compile.rs`: `Explained` → `std::process::exit(1)`, no output
- Tests: `NotFound` and `Explained` are distinct and must be tested separately

---

## State Transitions

```
IRIS_CONTAINER set
  └─ discover_via_docker_named(name)
       ├─ Docker daemon unreachable
       │    └─ DiscoveryResult::FoundUnhealthy(AtelierNotResponding) — special: daemon error
       ├─ Container not in list → DiscoveryResult::NotFound
       │    └─ discover_iris() continues cascade (Steps 4-6)
       ├─ Container found, port_web=None → DiscoveryResult::FoundUnhealthy(PortNotMapped)
       │    └─ discover_iris() returns IrisDiscovery::Explained (cascade stops)
       ├─ Container found, port mapped, probe → connection refused/timeout
       │    └─ DiscoveryResult::FoundUnhealthy(AtelierNotResponding { port })
       │    └─ discover_iris() returns IrisDiscovery::Explained (cascade stops)
       ├─ Container found, port mapped, probe → HTTP 4xx/5xx (not 401)
       │    └─ DiscoveryResult::FoundUnhealthy(AtelierHttpError { port, status })
       │    └─ discover_iris() returns IrisDiscovery::Explained (cascade stops)
       ├─ Container found, port mapped, probe → 401
       │    └─ DiscoveryResult::FoundUnhealthy(AtelierAuth401 { port })
       │    └─ discover_iris() returns IrisDiscovery::Explained (cascade stops)
       └─ Container found, port mapped, probe → 200 + valid JSON
            └─ DiscoveryResult::Connected(IrisConnection)
            └─ discover_iris() returns IrisDiscovery::Found(conn)
```

---

## Message Templates (per FailureMode)

Used by `discover_iris()` when it receives `FoundUnhealthy` from `discover_via_docker_named()`.

| FailureMode | Log level | Message template |
|-------------|-----------|-----------------|
| Docker daemon unreachable | `warn` | `Could not connect to Docker daemon — is Docker running? (IRIS_CONTAINER={name})` |
| `NotFound` | `warn` | `Container '{name}' not found in Docker — is it running? ('docker ps' to check)` |
| `PortNotMapped` | `warn` | `Container '{name}' found but port 52773 is not mapped to a host port. Restart with: docker run -p <host_port>:52773 ... Note: iris_execute and iris_test still work via docker exec.` |
| `AtelierNotResponding { port }` | `warn` | `Container '{name}' found at localhost:{port} but Atelier REST API is not responding. Enterprise IRIS images (iris:, irishealth:) do not include the private web server — use iris-community or irishealth-community for local dev, or connect via IRIS_HOST+IRIS_WEB_PORT to an external Web Gateway. Note: iris_execute and iris_test still work via docker exec.` |
| `AtelierHttpError { port, status }` | `warn` | `Container '{name}' found at localhost:{port} but Atelier REST returned HTTP {status}. Check IRIS logs: docker logs {name}` |
| `AtelierAuth401 { port }` | `warn` | *(Emitted by `probe_atelier_with_client` — no second message added)* |
