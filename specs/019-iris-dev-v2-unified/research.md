# Research: iris-dev v2 — Unified IRIS MCP Server

**Date**: 2026-04-19
**Branch**: 019-iris-dev-v2-unified

---

## Decision 1: HTTP Client Architecture

**Decision**: Single `reqwest::Client` created once at server startup, stored in `IrisTools`, shared across all tool calls.

**Rationale**: Current codebase creates a new client per call via `IrisConnection::http_client()`. For a long-running MCP server handling sequential tool calls, a single client with connection pooling is more efficient. reqwest's `Client` is `Clone + Send + Sync` and safe to share via `Arc`.

**Alternatives considered**: Per-call client (current) — wasteful; per-tool client — no benefit over shared.

---

## Decision 2: Atelier API Version Strategy

**Decision**: Try v8 first, fall back to v2 for search, fall back to v1 for everything else. Version detected once at startup via `GET /api/atelier/` fingerprint; stored in `IrisConnection.atelier_version`.

**Rationale**: v8 has the best compile/document API. v2 has the async search endpoint. v1 is the universal fallback for older IRIS builds. Nathan's TypeScript code uses v8 for documents and v2 for search — port this exactly.

**Atelier v8 compile request shape** (from Nathan's code):
```
POST /api/atelier/v8/{namespace}/action/compile
Body: {"docs": ["MyApp.Patient.cls"], "flags": "cuk"}
Response: {"result": {"console": [...], "errors": [...]}}
```

**Atelier v8 xecute shape** (existing in codebase):
```
POST /api/atelier/v8/{namespace}/action/xecute
Body: {"expression": "write $ZVERSION,!"}
Response: {"result": {"content": [{"name":"...","content":["line1","line2"]}]}}
```

**Atelier v2 search shape**:
```
GET /api/atelier/v2/{namespace}/action/search?query=...&regex=false&sys=false&category=CLS
Response: {"result": {"content": [{"doc":"MyApp.Cls","atLine":42,"member":"...","text":"..."}]}}
Async: POST with {"query":..} → {"result":{"workId":"abc"}} → poll GET /action/search?workId=abc
```

**Alternatives considered**: Always v1 — too conservative, loses Nathan's batch compile. Always v8 — breaks on pre-2021 IRIS.

---

## Decision 3: Multi-Action Tool Parameter Dispatch

**Decision**: Each multi-action tool (e.g., `iris_doc`, `iris_macro`, `interop_production`) dispatches on a required `mode` or `action` string field. The field uses `#[serde(default)]` only when there's a sensible default; otherwise it's `required`. Validated empirically: Haiku 4.5 invokes correctly 100% of the time (see spec clarifications).

**Pattern**:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IrisDocParams {
    pub mode: DocMode,   // "get" | "put" | "delete" | "head"
    pub name: String,
    pub content: Option<String>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DocMode { Get, Put, Delete, Head }
```

**Alternatives considered**: Separate tools per action — 60 tools, 3.2× token overhead, empirically no accuracy benefit.

---

## Decision 4: Subprocess Elimination Strategy

**Decision**: Replace all Python subprocess calls with direct Atelier REST calls:

| Tool | Old (subprocess) | New (direct) |
|------|-----------------|-------------|
| iris_compile | Python → bash script → native port | POST /action/compile |
| iris_execute | Python → bash script → native port | POST /action/xecute |
| iris_test | Python → bash script → %UnitTest | POST /action/xecute (parse output) |
| interop_* | Python → intersystems_iris native | POST /action/xecute (Ens.Director calls) |
| debug_source_map | Python → intersystems_iris native | POST /action/xecute (%Studio.Debugger) |

**xecute for interop**: `do ##class(Ens.Director).StartProduction("MyProd")` executes synchronously, returns output. For status: `write ##class(Ens.Director).GetProductionStatus(.name,.state,.itemcount)`. Parse output lines.

**Alternatives considered**: Keep Python subprocess as optional fallback — adds code complexity, perpetuates the bug surface. Rejected.

---

## Decision 5: Skills Registry Storage

**Decision**: In-memory ring buffer (last 50 tool calls) for session history. `^SKILLS` global for persistent skills, accessed via xecute. Minimum 5 calls before `skill_propose` is allowed.

**^SKILLS xecute pattern**:
```objectscript
// List: set key="" for key=^SKILLS { write key,"::",^SKILLS(key),! }
// Write: set ^SKILLS("name")=$lb(name,description,body,created)
// Delete: kill ^SKILLS("name")
```

**Alternatives considered**: File-based (`~/.iris-dev/skills.json`) — breaks multi-instance; IRIS SQL table — overkill for key-value skill store.

---

## Decision 6: Source Control Hook Invocation

**Decision**: When `IRIS_SOURCE_CONTROL=true`, `iris_doc(mode=put)` calls `OnBeforeSave()` via xecute using the same MCP connection credentials (IRIS_USERNAME/IRIS_PASSWORD). No separate SCM user. If hook returns error status, PUT is aborted.

**xecute pattern**:
```objectscript
set sc=##class(%Studio.SourceControl.ISC).OnBeforeSave("MyApp.Patient.cls")
if $system.Status.IsError(sc) { write "ERROR:",$system.Status.GetErrorText(sc) }
else { write "OK" }
```

---

## Decision 7: ETag Conflict Handling

**Decision**: Single automatic retry on HTTP 409. First request has no `If-None-Match` header. On 409, fetch current ETag via HEAD, retry PUT with `If-None-Match: <etag>`. If second attempt also fails, return structured conflict error.

**Rationale**: Matches Nathan's battle-tested TypeScript implementation. Single retry is sufficient for interactive AI-assisted editing where only one agent writes at a time.

---

## Decision 8: Discovery Race Fix

**Decision**: Replace 50ms sleep with `watch::Receiver::wait_for(|v| v.is_some())` with 5-second timeout. Already partially implemented in mcp.rs (this session). Ensures tools never return IRIS_UNREACHABLE due to a race where tools are called before discovery completes.

---

## Codebase Patterns (from research)

- **Tool macro**: `#[tool(description="...")]` on `async fn`, `Parameters<T>` for deserialization, `ok_json(v)` / `err_json(code, msg)` for responses
- **Error model**: Structured JSON responses preferred over throwing `McpError` — preserves error info for AI clients
- **Test pattern**: Spawn binary via `Command`, communicate via stdin/stdout `\n`-delimited JSON, 5-second response timeout per message
- **HTTP auth**: `.basic_auth(&self.username, Some(&self.password))` on every request
- **URL construction**: `format!("{}/api/atelier{}", base_url, path)` — base_url already includes optional prefix
