# Tasks: iris-dev v2 — Unified IRIS MCP Server

**Input**: Design documents from `/specs/019-iris-dev-v2-unified/`
**Branch**: `019-iris-dev-v2-unified`
**Total tasks**: 79
**Format**: `- [ ] [ID] [P?] [Story?] Description — file path`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Codebase hygiene and new type scaffolding that everything else builds on.

- [x] T001 Add `atelier_version: AtelierVersion` field and `AtelierVersion` enum (V8/V2/V1) to `IrisConnection` — `crates/iris-dev-core/src/iris/connection.rs`
- [x] T002 Add `path_prefix: Option<String>` field to `IrisConnection`; update `atelier_url()` to include prefix when set — `crates/iris-dev-core/src/iris/connection.rs`
- [x] T003 [P] Add `IRIS_WEB_PREFIX` env var field to `McpCommand` struct with `#[arg(long, env = "IRIS_WEB_PREFIX", default_value = "")]` — `crates/iris-dev-bin/src/cmd/mcp.rs`
- [x] T004 [P] Add `path_prefix` parsing to `vscode_config.rs` `to_iris_connection()` — reads `server.web_server.path_prefix` into `IrisConnection.path_prefix` — `crates/iris-dev-core/src/iris/vscode_config.rs`
- [x] T005 Add shared `Arc<reqwest::Client>` to `IrisTools` struct; create once at startup via `IrisConnection::http_client()`, store in struct — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T006 Replace `tokio::time::sleep(50ms)` with `iris_rx.wait_for(|v| v.is_some())` with 5s timeout in `McpCommand::run()` — `crates/iris-dev-bin/src/cmd/mcp.rs`
- [x] T007 Add `VecDeque<ToolCall>` ring buffer (capacity 50) to `IrisTools`; record tool name + success after each call — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T008 `cargo build` — must compile clean with no errors or new warnings

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: AtelierClient foundation and discovery fixes that all tool phases depend on.

**Independent test**: `cargo test discovery_waits_for_iris web_prefix_route_correct` both pass.

- [x] T009 Write failing test `discovery_waits_for_iris`: spawn server with no env vars, send initialize + tools/list, assert `tool_count=20` returned within 5s — `crates/iris-dev-core/tests/mcp_handshake.rs`
- [x] T010 Write failing test `web_prefix_route_correct`: mock Atelier endpoint at `/irisaicore/api/atelier`, start server with `IRIS_WEB_PREFIX=irisaicore IRIS_WEB_PORT=18080`, call `iris_compile`, assert request URL includes `/irisaicore/` — `crates/iris-dev-core/tests/integration/test_web_prefix.rs`
- [x] T011 Detect Atelier API version at startup: `GET /api/atelier/` fingerprint → parse `result.content[0].version` → set `conn.atelier_version` (V8 if version ≥ 2021, V2 if search available, else V1) — `crates/iris-dev-core/src/iris/connection.rs`
- [x] T012 Update `probe_atelier()` in discovery to include `path_prefix` in base_url construction — `crates/iris-dev-core/src/iris/discovery.rs`
- [x] T013 Pass `web_prefix` from `McpCommand` into explicit `IrisConnection` construction in `run()` — `crates/iris-dev-bin/src/cmd/mcp.rs`
- [x] T013a Update `iris_unreachable()` and all connection error paths to include the attempted URL and the env var to check (e.g., "Cannot reach http://localhost:52773 — set IRIS_WEB_PORT if using a non-standard port") — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T014 **Phase gate**: run `cargo test discovery_waits_for_iris web_prefix_route_correct` — both must pass before Phase 3

---

## Phase 3: US1 — Zero-Config Connect and Compile

**Story goal**: `iris_compile` works via Atelier REST with no Python, no subprocess.
**Independent test**: Start server, compile a class, assert structured errors returned. Python must not be in PATH.

- [x] T015 [US1] Write failing unit test `compile_params_deserialize`: assert `CompileParams{target:"MyApp.*.cls", flags:"cuk"}` deserializes correctly including wildcard — `crates/iris-dev-core/tests/unit/test_compile_params.rs`
- [x] T016 [US1] Write failing integration test `iris_compile_success`: compile `%Library.Base`, assert `success:true, errors:[]` — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T017 [US1] Write failing integration test `iris_compile_error`: compile a class with known syntax error, assert `success:false` and `errors[0].line > 0` — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T018 [US1] Write failing integration test `iris_compile_no_python`: set PATH to exclude python, compile a class, assert succeeds — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T019 [US1] Reimplement `iris_compile` via `POST /api/atelier/v8/{ns}/action/compile` with `{"docs":[target],"flags":flags}`; parse `result.console` array for error lines (format `"  1 ERROR #line: text"`) — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T020 [US1] Add wildcard expansion: if target contains `*`, resolve to matching class names via `GET /api/atelier/v8/{ns}/docs?category=CLS` before compile — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T021 [US1] Add `force_writable` support: if `force_writable=true`, xecute `do ##class(%Library.EnsembleMgr).EnableNamespace(ns,1)` before compile — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T022 [US1] Delete Python subprocess code from old `iris_compile` implementation — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T023 [US1] **Phase gate**: `cargo test iris_compile_success iris_compile_error iris_compile_no_python` — all must pass

---

## Phase 4: US4 — Execute and Query Without Python

**Story goal**: `iris_execute` and `iris_query` use Atelier REST directly, no subprocess.
**Independent test**: Python absent from PATH; `iris_execute("write $ZVERSION,!")` returns IRIS version; `iris_query` returns rows.

- [x] T024 [US4] Write failing integration test `iris_execute_basic`: execute `write $ZVERSION,!`, assert output contains "IRIS" — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T025 [US4] Write failing integration test `iris_execute_error`: execute code with `<UNDEFINED>`, assert `success:false, iris_error.id="UNDEFINED"` — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T026 [US4] Write failing integration test `iris_query_rows`: query `SELECT TOP 3 Name FROM %Dictionary.ClassDefinition`, assert `rows` array with 3 entries — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T027 [US4] Write failing integration test `iris_test_unitest`: run a trivial %UnitTest, assert `passed >= 1, failed = 0` — `crates/iris-dev-core/tests/integration/test_mcp_iris.rs`
- [x] T028 [US4] Reimplement `iris_execute` via `POST /api/atelier/v8/{ns}/action/xecute`; extract output from `result.content[0].content` lines; parse IRIS errors from Atelier error response into `iris_error` field — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T029 [US4] Reimplement `iris_query` via `POST /api/atelier/v8/{ns}/action/query`; return `result.content` as JSON rows array with column names as keys — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T030 [US4] Reimplement `iris_test` via xecute of `##class(%UnitTest.Manager).RunTest(pattern,"/noload/run")`; parse output lines for `Passed=N` and `Failed=N` totals — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T031 [US4] Delete Python subprocess code from old `iris_execute`, `iris_test` implementations — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T032 [US4] **Phase gate**: `cargo test iris_execute_basic iris_execute_error iris_query_rows iris_test_unitest` — all pass

---

## Phase 5: US2 — Document Access (iris_doc)

**Story goal**: Full document CRUD with ETag conflict handling and SCM hook support.
**Independent test**: get → modify → put → get roundtrip; SCM hooks fire when `IRIS_SOURCE_CONTROL=true`.

- [x] T033 [US2] Write failing unit test `doc_mode_deserialization`: assert `"get"/"put"/"delete"/"head"` all deserialize to correct `DocMode` variants — `crates/iris-dev-core/tests/unit/test_doc_params.rs`
- [x] T034 [US2] Write failing integration test `iris_doc_get`: fetch `%Library.Base.cls`, assert content contains "Class %Library.Base" — `crates/iris-dev-core/tests/integration/test_doc.rs`
- [x] T035 [US2] Write failing integration test `iris_doc_roundtrip`: put modified content to a test class, get it back, assert modification persists — `crates/iris-dev-core/tests/integration/test_doc.rs`
- [x] T036 [US2] Write failing integration test `iris_doc_head_exists`: head `%Library.Base.cls`, assert `exists:true` — `crates/iris-dev-core/tests/integration/test_doc.rs`
- [x] T037 [US2] Write failing integration test `iris_doc_head_missing`: head `IrisDevTest.DoesNotExist.cls`, assert `exists:false` — `crates/iris-dev-core/tests/integration/test_doc.rs`
- [x] T038 [US2] Add `DocMode` enum and `IrisDocParams` struct with serde + JsonSchema derives — `crates/iris-dev-core/src/tools/doc.rs` (new file)
- [x] T039 [US2] Implement `handle_get`: `GET /api/atelier/v8/{ns}/doc/{name}`, join `result.content[0].content` lines — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T040 [US2] Implement `handle_put`: `PUT /api/atelier/v8/{ns}/doc/{name}` with `{"enc":false,"content":[...lines...]}`, handle 409 with single ETag retry — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T041 [US2] Add SCM hook invocation to `handle_put`: when `IRIS_SOURCE_CONTROL=true`, xecute `OnBeforeSave(name)` before PUT; check status; xecute `OnAfterSave(name)` after success — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T041a [US2] Add `IRIS_SKIP_SOURCE_CONTROL` bypass to `handle_put`: when env var is true, append `?csp=1` to the PUT URL to skip all server-side SCM checks — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T042 [US2] Implement `handle_delete`: `DELETE /api/atelier/v8/{ns}/doc/{name}` — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T043 [US2] Implement `handle_head`: `HEAD /api/atelier/v8/{ns}/doc/{name}`, return `exists` + timestamp from response headers — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T044 [US2] Add batch support: `mode=get` with `names: Vec<String>` → parallel fetches; `mode=delete` with `names` → parallel deletes — `crates/iris-dev-core/src/tools/doc.rs`
- [x] T045 [US2] Wire `iris_doc` into `IrisTools` tool router with `#[tool]` macro — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T046 [US2] **Phase gate**: `cargo test iris_doc_get iris_doc_roundtrip iris_doc_head_exists iris_doc_head_missing` — all pass

---

## Phase 6: US3 — Full-Text Search (iris_search)

**Story goal**: Sync search with automatic async fallback; wildcard scope; regex support.
**Independent test**: Search for known symbol in small namespace, results within 3s; verify async polling fires on timeout.

- [x] T047 [US3] Write failing unit test `search_params_defaults`: assert unset `regex/case_sensitive` default to false, `category` defaults to "ALL" — `crates/iris-dev-core/tests/unit/test_search_params.rs`
- [x] T048 [US3] Write failing integration test `iris_search_sync`: search `"iris_compile"` in USER namespace, assert at least one result within 3s — `crates/iris-dev-core/tests/integration/test_search.rs`
- [x] T049 [US3] Write failing integration test `iris_search_category_filter`: search with `category="CLS"`, assert no MAC or INT results — `crates/iris-dev-core/tests/integration/test_search.rs`
- [x] T050 [US3] Add `SearchParams` struct with `query, regex, case_sensitive, category, documents, namespace` fields — `crates/iris-dev-core/src/tools/search.rs` (new file)
- [x] T051 [US3] Implement sync path: `GET /api/atelier/v2/{ns}/action/search?query=...&regex=...&sys=false&category=...` with 2s timeout — `crates/iris-dev-core/src/tools/search.rs`
- [x] T052 [US3] Implement async fallback: on sync timeout, `POST /api/atelier/v2/{ns}/action/search` to get `workId`, poll `GET ?workId=X` every 2s up to 5 min — `crates/iris-dev-core/src/tools/search.rs`
- [x] T053 [US3] Add `documents` wildcard filtering: pre-fetch matching doc names via `iris_info(what=documents)`, pass as scope filter — `crates/iris-dev-core/src/tools/search.rs`
- [x] T054 [US3] Truncate results at 200, set `truncated:true, total_found:N` in response — `crates/iris-dev-core/src/tools/search.rs`
- [x] T055 [US3] Wire `iris_search` into `IrisTools` tool router — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T056 [US3] **Phase gate**: `cargo test iris_search_sync iris_search_category_filter` — both pass

---

## Phase 7: US7 — Discovery, Listing, Macros, Introspection, Debug, Generation

**Story goal**: `iris_info`, `iris_symbols`, `iris_introspect`, `iris_macro`, `iris_debug`, `iris_generate` all working via Atelier REST.
**Independent test**: Each tool returns valid structured data from live IRIS.

- [x] T057 [P] [US7] Write failing integration tests `iris_info_documents`, `iris_info_metadata`, `iris_macro_list`, `iris_introspect_class`, `iris_debug_error_logs` — `crates/iris-dev-core/tests/integration/test_discovery_tools.rs`
- [x] T058 [P] [US7] Implement `iris_info` dispatcher: `what=documents` → `GET /api/atelier/v8/{ns}/docs`; `what=modified` → `.../docs/modified`; `what=namespace` → `.../`; `what=metadata` → `.../metadata`; `what=jobs` → `.../jobs`; `what=csp_apps/csp_debug/sa_schema` → respective endpoints — `crates/iris-dev-core/src/tools/info.rs` (new file)
- [x] T059 [P] [US7] Reimplement `iris_symbols`: Atelier SQL query on `%Dictionary.ClassDefinition` via `/action/query` first; fall back to tree-sitter local if no IRIS — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T060 [P] [US7] Implement `iris_introspect`: SQL queries on `%Dictionary.CompiledMethod`, `CompiledProperty`, `CompiledParameter`, `CompiledXData` for named class, joined into structured response — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T061 [P] [US7] Implement `iris_macro` dispatcher: `list` → `GET /macros`; `signature/location/definition/expand` → `POST /action/getmacro` with `{"macros":[{"name":...,"arguments":N}]}` — `crates/iris-dev-core/src/tools/macro_tools.rs` (new file)
- [x] T062 [P] [US7] Implement `iris_debug` dispatcher: `map_int` → xecute `%Studio.Debugger.SourceLine`; `error_logs` → SQL on `%SYSTEM.Error`; `capture` → SQL on error state; `source_map` → xecute mapping — `crates/iris-dev-core/src/tools/debug.rs` (new file)
- [x] T063 [P] [US7] Implement `iris_generate`: call LLM via `IRIS_GENERATE_CLASS_MODEL` env (litellm HTTP); optionally compile result via `iris_compile` — `crates/iris-dev-core/src/tools/generate.rs` (new file)
- [x] T064 [US7] Wire `iris_info`, `iris_macro`, `iris_debug`, `iris_generate` into `IrisTools` tool router — `crates/iris-dev-core/src/tools/mod.rs`
- [x] T065 [US7] **Phase gate**: `cargo test iris_info_documents iris_info_metadata iris_macro_list iris_introspect_class iris_debug_error_logs` — all pass

---

## Phase 8: US5 — Interoperability Tools

**Story goal**: All 9 interop tools via Atelier xecute + SQL, no native superserver.
**Independent test**: `interop_production(action=status)` returns production state from Ensemble namespace using only `IRIS_WEB_PORT`.

- [x] T066 [US5] Write failing integration tests `interop_status_running`, `interop_logs_basic`, `interop_queues_basic` (skip if `IRIS_ENSEMBLE_NAMESPACE` not set) — `crates/iris-dev-core/tests/integration/test_interop.rs`
- [x] T067 [US5] Implement `interop_production` dispatcher via xecute: `status` → parse `GetProductionStatus` output; `start/stop/update/needs_update/recover` → corresponding `Ens.Director` xecute calls — `crates/iris-dev-core/src/tools/interop.rs`
- [x] T068 [US5] Implement `interop_query` dispatcher via Atelier SQL: `logs` → SELECT from `Ens_Util.Log`; `queues` → SELECT queue depths; `messages` → SELECT from `Ens.MessageHeader` — `crates/iris-dev-core/src/tools/interop.rs`
- [x] T069 [US5] Add `NOT_ENSEMBLE` graceful degradation: if xecute returns `<UNDEFINED>Ens.Director`, return `{success:false, error_code:"NOT_ENSEMBLE"}` — `crates/iris-dev-core/src/tools/interop.rs`
- [x] T070 [US5] Delete Python subprocess code from old interop implementations — `crates/iris-dev-core/src/tools/interop.rs`
- [x] T071 [US5] **Phase gate**: `cargo test interop_status_running interop_logs_basic interop_queues_basic` (or skip with note if no Ensemble namespace available)

---

## Phase 9: US6 — Skills Registry and Learning Agent

**Story goal**: `skill`, `skill_community`, `kb`, `agent_info` via xecute + in-memory buffer.
**Independent test**: propose after 5 calls, list shows skill, forget removes it.

- [x] T072 [US6] Write failing integration tests `skill_propose_min_calls`, `skill_list_roundtrip`, `agent_info_stats` — `crates/iris-dev-core/tests/integration/test_skills.rs`
- [x] T073 [US6] Implement `skill` dispatcher: `list/describe/search` → xecute `^SKILLS` global reads; `forget` → xecute `Kill ^SKILLS(name)`; `propose` → require ≥5 session ring buffer calls, synthesize name+description, xecute `Set ^SKILLS(name)=...` — `crates/iris-dev-core/src/tools/skills.rs`
- [x] T074 [US6] Add `OBJECTSCRIPT_LEARNING=false` guard: return `LEARNING_DISABLED` for all skill/kb tools — `crates/iris-dev-core/src/tools/skills.rs`
- [x] T075 [US6] Implement `skill_community`: `list` → fetch GitHub manifest from subscribed repos; `install` → write skill to `^SKILLS` — `crates/iris-dev-core/src/tools/skills.rs`
- [x] T076 [US6] Implement `kb`: `index` → read files, write chunks to `^KBCHUNKS` via xecute; `recall` → BM25 substring search over `^KBCHUNKS` — `crates/iris-dev-core/src/tools/skills.rs`
- [x] T077 [US6] Implement `agent_info`: `stats` → skill count + ring buffer size; `history` → last N entries from ring buffer — `crates/iris-dev-core/src/tools/skills.rs`
- [x] T078 [US6] **Phase gate**: `cargo test skill_propose_min_calls skill_list_roundtrip agent_info_stats` — all pass

---

## Phase 10: Polish and Final Integration

**Purpose**: Verify all 20 tools, binary size, documentation, remove last Python references.

- [x] T079 Write E2E test `e2e_all_tools_respond`: initialize server, call all 20 tools with minimal valid inputs, assert none return `INTERNAL_ERROR` — `crates/iris-dev-core/tests/integration/test_e2e_all_tools.rs`
- [x] T080 Write E2E test `e2e_steve_web_prefix`: full compile+get_doc+put_doc+search workflow with `IRIS_WEB_PREFIX=irisaicore IRIS_WEB_PORT=80` against a mock or real prefixed endpoint — `crates/iris-dev-core/tests/integration/test_e2e_all_tools.rs`
- [x] T081 Verify `cargo build --release` produces binary; assert no `python` in `otool -L` / `ldd` output — CI script or manual check
- [x] T082 [P] Audit `tools/mod.rs` for any remaining `std::process::Command::new("python")` calls — delete all found
- [x] T083 [P] Update `README.md`: remove "pip install" from install instructions; add "single binary, no Python required" to overview
- [x] T084 [P] Update `TROUBLESHOOTING.md` in `objectscript-mcp` repo: note that Python objectscript-mcp is no longer required when using iris-dev v2
- [x] T085 Bump `iris-dev` version to `0.2.0` in workspace `Cargo.toml`
- [x] T086 Run full test suite `cargo test --all` — must pass with 0 failures
- [x] T087 **Final gate**: `e2e_all_tools_respond` passes; `startup_latency_p50` benchmark passes (<100ms)

---

## Dependencies

```
Phase 1 (Setup) → Phase 2 (Foundation) → Phase 3 (US1 compile)
                                        → Phase 4 (US4 execute/query) [can parallel with Phase 3]
                                        → Phase 5 (US2 doc CRUD) [can parallel with Phase 3/4]
Phase 3+4+5 done → Phase 6 (US3 search) [needs execute working]
Phase 5 done    → Phase 7 (US7 discovery/macros/debug) [needs doc pattern]
Phase 3+4 done  → Phase 8 (US5 interop) [needs execute pattern]
Phase 2 done    → Phase 9 (US6 skills) [needs xecute pattern, can start after Phase 2]
All phases done → Phase 10 (Polish)
```

## Parallel Execution Opportunities

- **Phases 3, 4, 5** can run in parallel once Phase 2 is complete (different tool files, no intra-phase deps)
- **Phase 7 subtasks** (T057–T063) are all parallel within the phase
- **Phase 10 polish tasks** (T082–T084) are parallel

## Implementation Strategy (MVP First)

**MVP** (Phases 1–4): AtelierClient + compile + execute + query. This alone unblocks Tim and Steve — no Python needed.

**Full P1** (+ Phases 5–6): Add document CRUD and search. Covers all priority-1 user stories.

**Complete** (+ Phases 7–10): All 20 tools, interop, skills, polished binary.
