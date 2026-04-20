# Feature Specification: iris-dev v2 — Unified IRIS MCP Server

**Feature Branch**: `019-iris-dev-v2-unified`
**Created**: 2026-04-19
**Status**: Draft

## Overview

Merge three separate IRIS MCP implementations (iris-dev Rust, objectscript-mcp Python, Nathan Keast's intersystems-mcp-atelier TypeScript) into a single elegant Rust binary. The result is a zero-dependency, cross-platform MCP server exposing **20 composable tools** covering every IRIS developer workflow: compile, test, execute, document CRUD, full-text search, introspection, macros, CSP, debug, Interoperability, and a learning agent.

**Why 20, not 60**: An empirical test against Haiku 4.5 (via Bedrock) with realistic noisy context confirmed that multi-action tools (`iris_doc(mode=get/put/delete/head)`, `iris_macro(action=list/signature/expand)`, `interop_production(action=status/start/stop)`) are invoked correctly 100% of the time — identical accuracy to dedicated single-purpose tools. The 20-tool design uses 229 description tokens vs 731 for 60 tools: a **3.2× context saving** with zero accuracy cost. Smaller tool lists also reduce LLM confusion when selecting between near-identical tools.

The finished system works identically for:
- **Tim Leavitt** — ISC internal, P4, no Docker, standard IRIS at localhost:52773
- **Steve P** — non-standard web prefix (irisaicore), port 80
- **Docker-based dev** — iris-devtester containers with dynamic ports

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Zero-Config Connect and Compile (Priority: P1)

A developer opens a project in VS Code, starts `iris-dev mcp`, and within 5 seconds can compile ObjectScript without any manual configuration. The server discovers the running IRIS instance automatically.

**Why this priority**: Connection and compilation are the entry point for every other workflow. If this doesn't work for Tim's environment (no Docker, standard install) or Steve's (non-standard prefix), the entire tool is useless.

**Independent Test**: Start `iris-dev mcp` with only `IRIS_HOST=localhost IRIS_WEB_PORT=52773` set. Ask Copilot/Claude to compile a class. It should return compiler output within 3 seconds with no Python installed.

**Acceptance Scenarios**:

1. **Given** `IRIS_HOST` and `IRIS_WEB_PORT` are set, **When** `iris-dev mcp` starts, **Then** the server connects to IRIS within 5 seconds and reports `tool_count=20`
2. **Given** no env vars are set, **When** the server starts, **Then** it scans localhost ports [52773, 41773, 51773, 8080] in parallel and connects to the first that responds with an IRIS fingerprint
3. **Given** Docker is not available, **When** the discovery cascade runs, **Then** it skips Docker detection without error and proceeds to VS Code settings.json within 50ms
4. **Given** `IRIS_WEB_PREFIX=irisaicore` and `IRIS_WEB_PORT=80`, **When** `iris_compile` is called, **Then** the request goes to `http://localhost:80/irisaicore/api/atelier/v8/USER/action/compile` and returns results
5. **Given** a class has compile errors, **When** `iris_compile` is called, **Then** the response includes `success: false`, structured error objects with line numbers, and the target name
6. **Given** `iris_compile` is called with `"Package.*.cls"`, **When** the compilation runs, **Then** all classes in the package are compiled in a single Atelier v8 batch request
7. **Given** Python is completely uninstalled, **When** any tool is called, **Then** all 20 tools respond without error

---

### User Story 2 — Read, Edit, and Write Documents (Priority: P1)

A developer uses Claude/Copilot to read an existing class, modify it, and write it back — all without leaving the chat. The server handles ETag conflicts and optionally invokes server-side source control hooks before writing.

**Why this priority**: Document CRUD is the core of Nathan's contribution and the most-requested missing capability in the current Python server. It's essential for any AI-assisted editing workflow.

**Independent Test**: Call `iris_get_doc` on a known class, modify its content, call `iris_put_doc` with the modified content. Verify the change persists via a second `iris_get_doc`. Repeat with `IRIS_SOURCE_CONTROL=true` and verify hooks fire.

**Acceptance Scenarios**:

1. **Given** a class exists in IRIS, **When** `iris_get_doc("MyApp.Patient.cls")` is called, **Then** the full source is returned as a string with document metadata
2. **Given** a document is fetched, **When** `iris_put_doc` is called with modified content, **Then** the document is saved and `iris_compile` succeeds on it immediately after
3. **Given** another process modified a document between fetch and put, **When** `iris_put_doc` is called, **Then** the ETag conflict is detected, the latest version fetched, and the write retried automatically
4. **Given** `IRIS_SOURCE_CONTROL=true`, **When** `iris_put_doc` is called, **Then** `##class(%Studio.SourceControl.ISC).OnBeforeSave()` is invoked via xecute before the PUT and `OnAfterSave()` after
5. **Given** `IRIS_SKIP_SOURCE_CONTROL=true`, **When** `iris_put_doc` is called, **Then** `?csp=1` is appended to the PUT URL, bypassing all SCM hooks
6. **Given** a document doesn't exist, **When** `iris_head_doc` is called, **Then** `{exists: false}` is returned without error
7. **Given** a list of document names, **When** `iris_get_docs` is called, **Then** all documents are returned in a single batch response

---

### User Story 3 — Full-Text Search Across Codebase (Priority: P1)

A developer asks "find all uses of ##class(HS.FHIR.DTL.Util.API.Transform)" and gets results in under 3 seconds, even against a large HealthShare codebase with 50,000+ classes.

**Why this priority**: Search is how AI assistants navigate large codebases. Nathan's async-fallback implementation is production-proven with 50+ tests; it must be ported faithfully.

**Independent Test**: Call `iris_search` with a known method name in a small namespace. Verify results include file names and line numbers within 3 seconds. Call on a large namespace — verify async fallback fires transparently.

**Acceptance Scenarios**:

1. **Given** a search query, **When** `iris_search` is called against a namespace with fewer than 10,000 classes, **Then** results are returned within 3 seconds
2. **Given** a large namespace, **When** the Atelier v2 sync search exceeds 2 seconds, **Then** the request automatically upgrades to async polling with a `workId`, polling every 2 seconds up to 5 minutes
3. **Given** `regex: true`, **When** `iris_search("Get.*Status", regex: true)` is called, **Then** regex matching is applied across all documents
4. **Given** `category: "CLS"`, **When** `iris_search` is called, **Then** only `.cls` documents are searched
5. **Given** `documents: ["HS.FHIR.*.cls"]`, **When** `iris_search` is called, **Then** only documents matching the wildcard pattern are searched
6. **Given** `case_sensitive: false`, **When** `iris_search("getorderstatus")` is called, **Then** results include matches regardless of case

---

### User Story 4 — Execute and Query Without Python (Priority: P1)

A developer runs arbitrary ObjectScript or SQL from the chat with no Python runtime installed. The server executes directly via Atelier REST.

**Why this priority**: Python subprocess delegation is the #1 source of `IRIS_UNREACHABLE` errors. Eliminating it makes the tool reliable in Tim and Steve's environments and any clean install.

**Independent Test**: Install `iris-dev` binary only, with Python completely absent. Call `iris_execute("write $ZVERSION,!")`. Call `iris_query("SELECT TOP 3 Name FROM %Dictionary.ClassDefinition")`. Both must return results.

**Acceptance Scenarios**:

1. **Given** no Python runtime is installed, **When** `iris_execute("write $ZVERSION,!")` is called, **Then** the IRIS version string is returned
2. **Given** a SQL SELECT, **When** `iris_query` is called, **Then** rows are returned as a JSON array with column names as keys
3. **Given** ObjectScript that raises an error, **When** `iris_execute` is called, **Then** the response includes `success: false`, the IRIS error code, domain, and message
4. **Given** `iris_test("MyApp.Tests")` is called, **When** %UnitTest.Manager runs, **Then** the response includes `passed`, `failed`, `total` counts and full trace output
5. **Given** a configured timeout is exceeded, **When** `iris_execute` runs, **Then** the response includes `error_code: "TIMEOUT"` with elapsed time

---

### User Story 5 — Interoperability Production Management (Priority: P2)

An Ensemble/IRIS Interoperability developer monitors and controls productions from the chat. All tools work via Atelier REST even when the native superserver port is firewalled.

**Why this priority**: Interop tools are a strong differentiator. They must degrade gracefully to Atelier xecute + SQL to work in Tim's environment.

**Independent Test**: In an Ensemble namespace, call `interop_production_status`. Then call `interop_logs(limit: 5)`. Both must work with only `IRIS_WEB_PORT` set.

**Acceptance Scenarios**:

1. **Given** a production is running, **When** `interop_production_status` is called, **Then** production name, state (Running/Stopped/Troubled), and item count are returned
2. **Given** only Atelier REST is available, **When** `interop_production_status` is called, **Then** it executes `##class(Ens.Director).GetProductionStatus()` via xecute and returns results
3. **Given** `interop_logs(item_name: "MyBP", log_type: "error", limit: 10)` is called, **Then** up to 10 error log entries are returned via SQL through Atelier query endpoint
4. **Given** a production is Troubled, **When** `interop_production_recover` is called, **Then** recovery is attempted and the new state is returned
5. **Given** `interop_message_search(source: "MyBS", limit: 20)` is called, **Then** up to 20 message headers are returned with SessionId, TimeCreated, Status

---

### User Story 6 — Skills Registry and Learning Agent (Priority: P2)

After a session of MCP tool use, a developer asks the agent to synthesize a reusable skill from tool-call patterns. Skills persist in `^SKILLS` and survive restarts.

**Why this priority**: Skills are the learning differentiator. The registry must work reliably via Atelier xecute with no Python dependency.

**Independent Test**: Call `skill_propose`. Verify a skill is written to `^SKILLS`. Call `skill_list` — verify it appears. Call `skill_forget` — verify it disappears.

**Acceptance Scenarios**:

1. **Given** skills exist in `^SKILLS`, **When** `skill_list` is called, **Then** all skills with name, description, and usage count are returned
2. **Given** recent tool calls have been made, **When** `skill_propose` is called, **Then** a synthesized skill is written to `^SKILLS` via xecute and returned
3. **Given** `skill_search("compile")` is called, **Then** skills whose name or description contains "compile" are returned
4. **Given** `skill_forget("my-skill")` is called, **Then** the skill is removed from `^SKILLS` and no longer appears in `skill_list`
5. **Given** `OBJECTSCRIPT_LEARNING=false`, **When** any skill tool is called, **Then** `error_code: "LEARNING_DISABLED"` is returned

---

### User Story 7 — Macro and Deep Class Introspection (Priority: P3)

A developer investigating unfamiliar code can list macros, find their definitions, expand them, and get deep class metadata including XData blocks.

**Why this priority**: Macro and introspection tools complete the Nathan integration. P3 because less frequently needed but important for completeness.

**Independent Test**: Call `iris_macro_list` in a namespace with macros. Verify names are returned. Call `iris_introspect("Ens.BusinessProcess")`. Verify methods, properties, XData blocks, and superclasses are all present.

**Acceptance Scenarios**:

1. **Given** a namespace with macros, **When** `iris_macro_list` is called, **Then** available macro names are returned
2. **Given** a macro name, **When** `iris_macro_location` is called, **Then** the source file and line number where it's defined are returned
3. **Given** `iris_introspect("Ens.BusinessProcess")` is called, **Then** the response includes methods with signatures, properties with types, XData blocks by name, and the full superclass chain
4. **Given** `iris_get_jobs` is called, **Then** active IRIS jobs with PID, state, and namespace are returned

---

### Edge Cases

- **IRIS unreachable**: All tools return `{success: false, error_code: "IRIS_UNREACHABLE", error: "Cannot reach http://..., check IRIS_WEB_PORT"}`
- **Missing web prefix**: If `iris_put_doc` returns 404 and `IRIS_WEB_PREFIX` is unset, error message includes "check IRIS_WEB_PREFIX setting"
- **Docker absent**: Discovery cascade skips bollard probe silently, proceeds within 50ms
- **Atelier version mismatch**: Client retries with v2, then v1 on 404 from v8
- **ETag conflict**: Single automatic retry; if second attempt fails, return structured conflict error
- **Non-existent compile target**: `{success: false, error_code: "NOT_FOUND", target: "..."}`
- **Large search results**: Truncated at 200 results with `truncated: true` and `total_found` count
- **SCM hook rejects write**: If `OnBeforeSave()` returns error status, PUT is aborted and SCM error message is returned
- **Interop on non-Ensemble namespace**: `{success: false, error_code: "NOT_ENSEMBLE", error: "Interoperability not available in namespace USER"}`
- **Skills namespace missing**: If `OBJECTSCRIPT_SKILLMCP_NAMESPACE` namespace doesn't exist, `skill_list` returns empty array rather than error

---

## Requirements *(mandatory)*

### Functional Requirements

**Connection & Discovery**

- **FR-001**: The server MUST connect to IRIS using only Atelier REST — no Python runtime, no native superserver required
- **FR-002**: The server MUST support `IRIS_WEB_PREFIX` for non-root gateway installations
- **FR-003**: Discovery MUST be non-blocking — Docker unavailability is detected and skipped within 50ms
- **FR-004**: The server MUST wait up to 5 seconds for discovery before accepting tool calls (watch channel, not sleep)
- **FR-005**: The server MUST read `objectscript.conn` and `intersystems.servers` (including `pathPrefix`) from VS Code settings files

**Document Access**

- **FR-006**: `iris_get_doc` MUST support all IRIS document types: cls, mac, int, inc, csp, dfi, lut
- **FR-007**: `iris_put_doc` MUST handle ETag conflicts with a single automatic retry
- **FR-008**: `iris_put_doc` MUST invoke SCM hooks via xecute when `IRIS_SOURCE_CONTROL=true`, using the same IRIS_USERNAME/IRIS_PASSWORD credentials as the MCP connection — no separate SCM user
- **FR-009**: `iris_put_doc` MUST append `?csp=1` when `IRIS_SKIP_SOURCE_CONTROL=true`
- **FR-010**: Batch operations MUST use single Atelier batch requests, not N individual calls

**Compilation, Execution, Testing**

- **FR-011**: `iris_compile` MUST use Atelier v8 `/action/compile` with no subprocess
- **FR-012**: `iris_compile` MUST support wildcard package targets (`"Pkg.*.cls"`)
- **FR-013**: `iris_compile` MUST return structured error objects with line number, column, code, and severity per message
- **FR-014**: `iris_execute` MUST use Atelier v8 `/action/xecute` with no subprocess
- **FR-015**: `iris_query` MUST use Atelier v8 `/action/query` and return rows as JSON array
- **FR-016**: `iris_test` MUST run %UnitTest.Manager via xecute and return structured pass/fail/total counts

**Search**

- **FR-017**: `iris_search` MUST use Atelier v2 sync first, auto-upgrade to async polling when sync exceeds 2 seconds
- **FR-018**: `iris_search` MUST support `regex`, `case_sensitive`, `category`, and `documents` wildcard parameters
- **FR-019**: Async search polling MUST continue for up to 5 minutes at 2-second intervals

**Interoperability**

- **FR-020**: All `interop_*` tools MUST work via Atelier xecute and SQL query without native superserver
- **FR-021**: `interop_logs` and `interop_queues` MUST fall back to SQL queries when xecute is unavailable

**Skills & Learning**

- **FR-022**: `^SKILLS` MUST be read/written via Atelier xecute in `OBJECTSCRIPT_SKILLMCP_NAMESPACE`
- **FR-023**: All skill tools MUST return `LEARNING_DISABLED` error when `OBJECTSCRIPT_LEARNING=false`
- **FR-024**: `skill_propose` MUST mine session tool-call patterns and write a synthesized skill to `^SKILLS`

**Error Handling**

- **FR-025**: All tools MUST return `{success, error?, error_code?, iris_error?: {code, domain, id, params}}`
- **FR-026**: IRIS error structures from Atelier responses MUST be preserved in `iris_error`, not flattened
- **FR-027**: Connection failures MUST include the attempted URL and the env var to check

**Distribution**

- **FR-028**: The artifact MUST be a single static binary with no Python, Node.js, or runtime dependencies
- **FR-029**: `cargo build --release` MUST be the only build step required

### Tool Inventory (20 tools)

| Tool | `action`/`mode` params | Replaces |
|------|------------------------|---------|
| `iris_compile` | — | iris_compile |
| `iris_execute` | — | iris_execute |
| `iris_query` | — | iris_query |
| `iris_test` | — | iris_test |
| `iris_search` | — | iris_search |
| `iris_doc` | `mode`: get, put, delete, head | iris_get_doc, iris_put_doc, iris_delete_doc, iris_head_doc, iris_get_docs, iris_delete_docs |
| `iris_symbols` | — | iris_symbols, iris_symbols_local |
| `iris_introspect` | — | iris_introspect + %Dictionary deep query |
| `iris_macro` | `action`: list, signature, location, definition, expand | all 5 macro tools |
| `iris_info` | `what`: documents, modified, namespace, metadata, jobs, csp_apps, csp_debug, sa_schema | 8 discovery/listing tools |
| `iris_debug` | `action`: map_int, capture, error_logs, source_map | all 4 debug tools |
| `iris_generate` | `type`: class, test | iris_generate_class, iris_generate_test |
| `interop_production` | `action`: status, start, stop, update, needs_update, recover | all 6 production lifecycle tools |
| `interop_query` | `what`: logs, queues, messages | interop_logs, interop_queues, interop_message_search |
| `skill` | `action`: list, describe, search, forget, propose | skill_list/describe/search/forget/propose |
| `skill_community` | `action`: list, install | skill_community_list, skill_community_install |
| `kb` | `action`: index, recall | kb_index, kb_recall |
| `agent_info` | `what`: history, stats | agent_history, agent_stats |
| `iris_containers` | `action`: list, select, start | iris_list_containers, iris_select_container, iris_start_sandbox |
| `iris_symbols_local` | — | iris_symbols_local (kept separate: no IRIS required) |

**Note**: `iris_symbols_local` stays dedicated because it operates on local filesystem with no IRIS connection — merging it into `iris_symbols` would conflate two fundamentally different data sources.

### Key Entities

- **IrisConnection**: `{base_url (with prefix), namespace, username, password, atelier_version, discovery_source}`
- **AtelierClient**: Stateless reqwest wrapper — URL construction, auth, version negotiation, ETag tracking, response deserialization
- **ToolError**: `{code: ErrorCode, message: String, iris_error: Option<IrisError>, details: Option<Value>}`
- **IrisError**: `{code: i32, domain: String, id: String, params: Vec<String>}` — preserved from Atelier error response
- **Skill**: `{name, description, body, usage_count, created_at}` — stored as JSON in `^SKILLS(name)`
- **SearchResult**: `{document, line, content, match_type}` — one per match from `iris_search`

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 20 tools respond successfully against a standard IRIS install with no Docker and no Python (Tim's environment)
- **SC-002**: `iris_compile`, `iris_execute`, `iris_query`, and `iris_test` work with Python completely absent from the system
- **SC-003**: Server startup time (initialize response) under 100ms at p50 across 5 runs — preserves existing benchmark
- **SC-004**: `iris_search` returns results in under 3 seconds for namespaces under 10,000 classes; larger namespaces complete via async fallback without user-visible failure
- **SC-005**: `iris_put_doc` with `IRIS_SOURCE_CONTROL=true` rejects writes when the SCM hook returns an error — no silent bypasses
- **SC-006**: Steve's environment (`IRIS_WEB_PREFIX=irisaicore`, `IRIS_WEB_PORT=80`) passes `iris_compile`, `iris_get_doc`, `iris_put_doc`, and `iris_search`
- **SC-007**: All existing iris-dev unit and integration tests pass on the v2 branch
- **SC-008**: New integration tests cover: compile, execute, query, get_doc, put_doc (with/without prefix), search (sync and async paths), interop_production_status, skill_list
- **SC-009**: Tim can configure the MCP server with three env vars (`IRIS_HOST`, `IRIS_WEB_PORT`, `IRIS_USERNAME`/`IRIS_PASSWORD`) and have all tools functional — no Python install, no objectscript-mcp package
- **SC-010**: `cargo build --release` produces a working binary; no additional installation steps required

---

## Clarifications

### Session 2026-04-19

- Q: Should the server expose 60 dedicated tools or ~20 composable multi-action tools? → A: 20 composable tools. Empirical test against Haiku 4.5 with realistic noisy context showed 100% correct invocation of multi-action tools (same as dedicated), at 229 description tokens vs 731 — a 3.2× context saving with zero accuracy cost.
- Q: Where are session tool-call patterns stored for `skill_propose`? → A: In-memory ring buffer (last 50 calls); propose requires minimum 5 calls in session.
- Q: What credentials does the `iris_put_doc` SCM hook xecute use? → A: Same credentials as the MCP connection (IRIS_USERNAME/IRIS_PASSWORD). No separate SCM user required.

---

## Assumptions

1. Atelier REST v8 is available on all supported IRIS versions (2020.1+); v2/v1 fallback handles older builds.
2. Tim's IRIS is accessible at localhost:52773 with no path prefix.
3. Nathan's async search (`workId` polling) behavior is sufficiently documented in his TypeScript implementation to port to Rust without behavioral regression.
4. Interop tools via xecute are acceptably fast for interactive use (under 2 seconds for status/start/stop).
5. The `^SKILLS` global is retained as the skill store; IRIS vector store for KB is deferred to a follow-on spec.
6. `skill_optimize` (DSPy) and `skill_share` (GitHub API) remain `NOT_IMPLEMENTED` stubs — out of scope for this spec.
7. The objectscript-mcp Python package is not deprecated immediately; it continues to exist but is no longer a runtime dependency of iris-dev.
8. The VSCode extension (`vscode-objectscript-mcp`) already reads `pathPrefix` from `intersystems.servers` and passes it as `IRIS_WEB_PREFIX` (fix merged in this session).
