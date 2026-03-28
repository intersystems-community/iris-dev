# Spec 018 — IRIS Discovery Parity with objectscript-mcp

**Status**: Pending (blocked on objectscript-mcp spec-025 stabilization)  
**Author**: Thomas Dyar  
**Reference**: `objectscript-coder/specs/025-iris-discovery/spec.md`  
**Motivation**: iris-dev has a 5-step discovery cascade in `discovery.rs` but has
the same multi-container disambiguation bug as objectscript-mcp did. Bring it to
parity with the fixes shipped in objectscript-mcp spec-025 Phase 1.

---

## What objectscript-mcp shipped (spec-025 Phase 1)

These are the concrete changes to port:

| Change | objectscript-mcp file | iris-dev target |
|--------|----------------------|-----------------|
| `iris_list_containers` tool | `handlers/iris_list_containers.py` | new tool in `crates/iris-dev-core/src/tools/` |
| `iris_select_container` tool | `handlers/iris_select_container.py` | new tool in `crates/iris-dev-core/src/tools/` |
| `reconnect_to()` runtime swap | `connection.py` | extend `IrisConnection` / server state in `crates/iris-dev-core/src/iris/` |
| `IRIS_NOT_FOUND` with candidates | `handlers/iris_symbols.py` | any tool that currently returns a bare connection error |
| Missing import bugfix | `server.py` | N/A (Rust, won't apply) |

---

## Discovery cascade fixes needed in iris-dev

Current `discovery.rs` bug: `discover_via_docker()` returns the **first** matching
container (line 148 `return Some(conn)`). With 10 containers running this is a
coin flip.

Fix: replace first-match with scored name-match (same algorithm as
`iris_list_containers._score()`):

```rust
// Score a container name against workspace basename
fn score_container(container_name: &str, workspace_basename: &str) -> u32 {
    let cn = container_name.to_lowercase().trim_start_matches('/').to_owned();
    let wb = workspace_basename.to_lowercase();
    let mut score = 0u32;
    if cn == wb { score = 100; }
    else if cn.starts_with(&wb) { score = 80; }
    else if cn.contains(&wb) { score = 60; }
    if cn.ends_with("-iris") || cn.ends_with("_iris") { score += 10; }
    if cn.ends_with("-test") || cn.ends_with("_test") { score += 5; }
    score
}
```

Collect all candidates, sort by score descending, take the highest. If score == 0
(no match), fall through to next discovery step rather than returning first container.

---

## New tools to add

### `iris_list_containers`

```rust
// Returns all running IRIS containers with name, ports, image, status, age, score.
// No IRIS connection required.
// Uses bollard (already a dependency) — no iris-devtester subprocess needed.
async fn iris_list_containers(ctx: &ToolContext) -> ToolResult
```

Scoring against `ctx.workspace_root.file_name()`.

### `iris_select_container`

```rust
// Reconnect server to a named container for this session.
// Drops current IrisConnection, resolves port via bollard, reconnects.
async fn iris_select_container(name: String, namespace: Option<String>, ctx: &ToolContext) -> ToolResult
```

Mutates the server's `Arc<Mutex<Option<IrisConnection>>>` (or equivalent).

---

## Port duality

iris-dev uses Atelier web port (52773) exclusively today. After this spec, also
record the superserver port (1972-mapped) in `IrisConnection` — needed for any
future DBAPI work and for `iris_select_container` to report both ports.

Add to `IrisConnection`:
```rust
pub port_superserver: Option<u16>,  // mapped host port for 1972/tcp
pub port_web: u16,                  // existing — mapped host port for 52773/tcp
```

---

## When to do this

After objectscript-mcp spec-025 is fully stable (Phases 1-3 passing).
iris-dev Phase 5 from spec-025.

Expected effort: ~1 day (Rust port of ~200 lines of Python).
