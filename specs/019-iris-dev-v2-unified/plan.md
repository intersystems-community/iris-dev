# Implementation Plan: iris-dev v2 — Unified IRIS MCP Server

**Branch**: `019-iris-dev-v2-unified` | **Date**: 2026-04-19 | **Spec**: [spec.md](spec.md)

## Summary

Replace Python subprocess delegation in iris-dev with direct Atelier REST v8 calls, merge Nathan Keast's 38-tool TypeScript Atelier server into Rust, and collapse 60 potential tool definitions into 20 composable multi-action tools. The result is a single static Rust binary with zero runtime dependencies that works in Tim Leavitt's (no Docker/Python), Steve P's (web prefix), and Docker-based developer environments.

**Key architectural change**: Every tool that previously called `python3 -c "from objectscript_mcp.handlers..."` is replaced with a direct `reqwest` HTTP call to an Atelier REST endpoint.

---

## Technical Context

**Language/Version**: Rust 2021, tokio async runtime
**Primary Dependencies**: rmcp 1.2 (macros, server, schemars, transport-io), reqwest 0.12 (async HTTP), bollard 0.17 (Docker), serde + serde_json, tokio 1.x
**Storage**: IRIS `^SKILLS` global (via xecute), in-memory ring buffer for session history
**Testing**: `cargo test` — unit tests + integration tests spawning the binary
**Target Platform**: macOS (arm64/x86_64), Linux (x86_64/arm64), Windows (x86_64) — single static binary
**Performance Goals**: Startup (initialize response) p50 < 100ms; tool call round-trip < 3s for standard ops
**Constraints**: No Python, no Node.js, no runtime dependencies; single binary from `cargo build --release`

---

## Constitution Check

*No formal constitution defined for this project. Operating under spec-derived principles:*

- **Test-first**: Integration tests written for each phase before implementation
- **No Python subprocess**: FR-011/014 — hard gate, compile fails if any `std::process::Command::new("python")` exists in tools/
- **Single binary**: FR-028/029 — `cargo build --release` only
- **Structured errors**: All tools return JSON with `success` field; no bare panics

---

## Phase 1: AtelierClient Foundation + Discovery Fix (P1 unblock)

**Goal**: Shared HTTP client, web prefix support, discovery race fix. Everything else builds on this.

### Tasks

1. **[TEST]** Write failing integration test: `discovery_waits_for_iris` — starts server with no env vars, sends initialize + tools/list, asserts tool_count=20 returned within 5s
2. **[TEST]** Write failing integration test: `web_prefix_compile` — starts server with `IRIS_WEB_PREFIX=irisaicore IRIS_WEB_PORT=80`, calls iris_compile, asserts request URL includes `/irisaicore/api/atelier`
3. Add `path_prefix: Option<String>` field to `IrisConnection`; update `atelier_url()` to include prefix: `format!("{}/{}/api/atelier{}", base, prefix.trim_matches('/'), path)`
4. Add `AtelierVersion` enum (V8/V2/V1) to `IrisConnection`; detect at startup via `GET /api/atelier/` fingerprint
5. Add shared `reqwest::Client` to `IrisTools` struct (created once, stored as `Arc<reqwest::Client>`)
6. Fix discovery race: replace `tokio::time::sleep(50ms)` with `iris_rx.wait_for(|v| v.is_some())` with 5s timeout in `mcp.rs`
7. Add `IRIS_WEB_PREFIX` env var parsing to `McpCommand` and named-server discovery in `vscode_config.rs`
8. Run tests — both must pass

**E2E gate**: `cargo test discovery_waits_for_iris web_prefix_compile` passes.

---

## Phase 2: Eliminate Python Subprocess — Core Tools (P1)

**Goal**: `iris_compile`, `iris_execute`, `iris_query`, `iris_test` use Atelier REST directly.

### Tasks

1. **[TEST]** Write failing integration tests for each tool (requires live IRIS via `IRIS_WEB_PORT`):
   - `iris_compile_success`: compile a known-good class, assert `success:true`
   - `iris_compile_error`: compile a class with a syntax error, assert structured error with line number
   - `iris_execute_basic`: execute `write $ZVERSION,!`, assert output contains "IRIS"
   - `iris_query_basic`: SELECT TOP 1 from %Dictionary.ClassDefinition, assert rows returned
   - `iris_test_basic`: run a trivial %UnitTest class, assert `passed > 0`
2. Implement `iris_compile` via `POST /api/atelier/v8/{ns}/action/compile` — parse `result.console` for errors (format: `"ERROR #line: text"`)
3. Implement `iris_execute` via `POST /api/atelier/v8/{ns}/action/xecute` — return `result.content[0].content` joined as output string
4. Implement `iris_query` via `POST /api/atelier/v8/{ns}/action/query` — return `result.content` as JSON array
5. Implement `iris_test` via xecute of `##class(%UnitTest.Manager).RunTest(pattern,"/noload/run")` — parse output for `PASSED=N FAILED=N`
6. Delete Python subprocess code from `iris_compile`, `iris_execute`, `iris_test` in `tools/mod.rs`
7. Add `force_writable` support to `iris_compile`: if set, call `do ##class(%Library.EnsembleMgr).EnableNamespace(ns,1)` via xecute before compile
8. Run tests — all 5 must pass

**E2E gate**: `cargo test iris_compile_success iris_compile_error iris_execute_basic iris_query_basic iris_test_basic` passes against live IRIS.

---

## Phase 3: Document Access — iris_doc (P1)

**Goal**: Full document CRUD via Atelier v8 with ETag conflict handling and SCM hook support.

### Tasks

1. **[TEST]** Write failing integration tests:
   - `iris_doc_get`: fetch source of a known class, assert content contains "Class"
   - `iris_doc_put_get_roundtrip`: put modified content, get it back, assert change persists
   - `iris_doc_head_exists`: head a known class, assert `exists:true`
   - `iris_doc_head_missing`: head a non-existent class, assert `exists:false`
   - `iris_doc_put_conflict`: simulate ETag conflict (concurrent write), assert automatic retry
   - `iris_doc_scm_hooks`: with `IRIS_SOURCE_CONTROL=true`, put doc, assert xecute called OnBeforeSave
2. Implement `DocMode` enum and `IrisDocParams` struct
3. Implement `iris_doc` dispatcher: route to `handle_get`, `handle_put`, `handle_delete`, `handle_head`
4. `handle_get`: `GET /api/atelier/v8/{ns}/doc/{name}` — decode content lines array to string
5. `handle_put`: `PUT /api/atelier/v8/{ns}/doc/{name}` with `{"enc":false,"content":[...lines...]}` — handle 409 with single ETag retry; invoke SCM hooks when `IRIS_SOURCE_CONTROL=true`
6. `handle_delete`: `DELETE /api/atelier/v8/{ns}/doc/{name}`
7. `handle_head`: `HEAD /api/atelier/v8/{ns}/doc/{name}` — return exists + timestamp from headers
8. Batch support: `mode=get` with `names: Vec<String>` — parallel fetches; `mode=delete` with names — batch DELETE
9. Run tests

**E2E gate**: All 6 iris_doc tests pass.

---

## Phase 4: Search, Discovery & Listing (P1/P2)

**Goal**: `iris_search` with async fallback; `iris_info` for namespace/document discovery.

### Tasks

1. **[TEST]** Write failing integration tests:
   - `iris_search_sync`: search for a known symbol in small namespace, assert results within 3s
   - `iris_search_async_fallback`: mock/inject 2s delay on sync endpoint, assert async polling fires
   - `iris_info_documents`: `iris_info(what=documents)`, assert list returned
   - `iris_info_metadata`: `iris_info(what=metadata)`, assert IRIS version in response
2. Implement `iris_search`:
   - Try `GET /api/atelier/v2/{ns}/action/search?query=...` with 2s timeout
   - On timeout: `POST /action/search` to get workId, poll `GET ?workId=X` every 2s up to 5 min
   - Support `regex`, `case_sensitive`, `category`, `documents` params
   - Truncate at 200 results, set `truncated:true`
3. Implement `iris_info` dispatcher: route on `what` param
   - `documents`: `GET /api/atelier/v8/{ns}/docs?category={type}`
   - `modified`: `GET /api/atelier/v8/{ns}/docs/modified`
   - `namespace`: `GET /api/atelier/v8/{ns}`
   - `metadata`: `GET /api/atelier/v8/{ns}/metadata`
   - `jobs`: `GET /api/atelier/v8/{ns}/jobs`
   - `csp_apps`: `GET /api/atelier/v8/{ns}/cspapps`
   - `csp_debug`: `GET /api/atelier/v8/{ns}/cspdebugid`
   - `sa_schema`: `GET /api/atelier/v8/{ns}/saschema/{name}`
4. Run tests

**E2E gate**: `iris_search_sync`, `iris_info_documents`, `iris_info_metadata` pass.

---

## Phase 5: Symbols, Introspection, Macros, Debug (P2/P3)

**Goal**: `iris_symbols`, `iris_introspect`, `iris_macro`, `iris_debug` natively via Atelier.

### Tasks

1. **[TEST]** Write failing tests:
   - `iris_symbols_basic`: search for "%Library.Base", assert result found
   - `iris_introspect_class`: introspect "Ens.BusinessProcess", assert methods + properties present
   - `iris_macro_list`: list macros in namespace, assert non-empty
   - `iris_debug_error_logs`: fetch error logs, assert structure valid
2. Implement `iris_symbols`: Atelier SQL query on `%Dictionary.ClassDefinition` via `/action/query` — 3-tier: SQL first, then tree-sitter local fallback
3. Implement `iris_introspect`: SQL queries on `%Dictionary.CompiledMethod`, `CompiledProperty`, `CompiledParameter`, `CompiledXData` for the named class
4. Implement `iris_macro` dispatcher:
   - `list`: `GET /api/atelier/v8/{ns}/macros`
   - `signature/location/definition/expand`: `POST /api/atelier/v8/{ns}/action/getmacro` with `{"macros":[{"name":"...","arguments":N}]}`
5. Implement `iris_debug` dispatcher:
   - `map_int`: xecute `##class(%Studio.Debugger).SourceLine(routine, offset)` — parse result
   - `error_logs`: SQL query on `%SYSTEM.Error` via `/action/query`
   - `capture`: SQL query on current error state
   - `source_map`: build .INT→.CLS mapping via xecute
6. Implement `iris_generate` (class + test): LLM call via `IRIS_GENERATE_CLASS_MODEL` env var using litellm, then optional compile via `iris_compile`
7. Run tests

**E2E gate**: symbols, introspect, macro_list, debug_error_logs all pass.

---

## Phase 6: Interoperability Tools (P2)

**Goal**: All 9 interop tools via Atelier xecute + SQL, no native superserver required.

### Tasks

1. **[TEST]** Write failing tests (require Ensemble-enabled IRIS namespace):
   - `interop_production_status_running`: assert production name and state returned
   - `interop_logs_basic`: assert log entries returned
   - `interop_queues_basic`: assert queue list returned
2. Reimplement `interop_production` dispatcher via xecute:
   - `status`: xecute `set rc=##class(Ens.Director).GetProductionStatus(.name,.state,.count) write name,"|",state,"|",count`
   - `start`: xecute `do ##class(Ens.Director).StartProduction(prod)`
   - `stop`: xecute `do ##class(Ens.Director).StopProduction(timeout,force)`
   - `update/needs_update/recover`: corresponding xecute calls
3. Reimplement `interop_query` via Atelier SQL query:
   - `logs`: SELECT from `Ens_Util.Log` ordered by ID DESC
   - `queues`: SELECT from Ens queue enumeration via SQL
   - `messages`: SELECT from `Ens.MessageHeader`
4. Degrade gracefully: if xecute returns NOT_ENSEMBLE error, return structured error
5. Delete Python subprocess interop code
6. Run tests

**E2E gate**: interop_production_status, interop_logs_basic, interop_queues_basic pass on Ensemble namespace.

---

## Phase 7: Skills Registry + Learning Agent (P2)

**Goal**: `skill`, `skill_community`, `kb`, `agent_info` via xecute + in-memory buffer.

### Tasks

1. **[TEST]** Write failing tests:
   - `skill_propose_min_calls`: call 5 tools, call skill_propose, assert skill written to ^SKILLS
   - `skill_list_roundtrip`: propose skill, list, assert it appears; forget, list again, assert gone
   - `agent_info_stats`: assert skill_count and session_calls in response
2. Add `VecDeque<ToolCall>` (capacity 50) to `IrisTools` — record every tool call
3. Implement `skill` dispatcher via xecute:
   - `list`: iterate `^SKILLS` global, return array
   - `describe/forget`: `$Get(^SKILLS(name))` / `Kill ^SKILLS(name)`
   - `search`: iterate ^SKILLS, substring match on name+description
   - `propose`: require ≥5 session calls; use last N calls as pattern; write synthesized skill via `Set ^SKILLS(name)=...`
4. Implement `skill_community` (list: GitHub manifest fetch; install: write to ^SKILLS)
5. Implement `kb` (index: read files, write to ^KBCHUNKS via xecute; recall: BM25 substring search)
6. Implement `agent_info` (stats from skill count + ring buffer; history from ring buffer)
7. Add `OBJECTSCRIPT_LEARNING=false` guard — return `LEARNING_DISABLED` for all skill/kb tools
8. Run tests

**E2E gate**: skill_propose_min_calls, skill_list_roundtrip pass.

---

## Phase 8: Container Tools + Final Integration (P3)

**Goal**: `iris_containers` works cleanly; all 20 tools tested end-to-end; binary size verified.

### Tasks

1. **[TEST]** Write failing tests:
   - `e2e_all_tools_no_python`: assert all 20 tools respond (no INTERNAL_ERROR) against live IRIS with Python uninstalled (or simply not in PATH)
   - `e2e_steve_web_prefix`: full workflow (compile, get_doc, put_doc, search) with `IRIS_WEB_PREFIX=irisaicore IRIS_WEB_PORT=80`
   - `startup_latency_p50`: existing benchmark — assert < 100ms
2. Verify `iris_containers` works: list/select/start via iris-devtester CLI
3. `cargo build --release` — assert binary size < 20MB, no Python/Node deps in `ldd` output
4. Update README and TROUBLESHOOTING.md to reflect v2 (no Python install needed)
5. Update VSCode extension: bump version, verify `IRIS_WEB_PREFIX` env var pass-through working (already fixed in this session)
6. Run full test suite: `cargo test --all`
7. Run all 20 tools manually against iris-dev-iris container

**E2E gate**: `e2e_all_tools_no_python` passes; `startup_latency_p50` passes; `cargo build --release` succeeds.

---

## Artifacts

- `specs/019-iris-dev-v2-unified/research.md` ✓
- `specs/019-iris-dev-v2-unified/data-model.md` ✓
- `specs/019-iris-dev-v2-unified/contracts/tool-contracts.md` ✓
- `specs/019-iris-dev-v2-unified/plan.md` (this file) ✓
