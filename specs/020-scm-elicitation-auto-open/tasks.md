# Tasks: iris-dev v2.1 — SCM Elicitation, Auto-Open, SCM Tools

**Branch**: `020-scm-elicitation-auto-open`
**Format**: `- [ ] [ID] [P?] [Story?] Description — file path`

---

## Phase 1: Setup

- [ ] T001 Add `uuid` and `dirs` crates to `[dependencies]` in `crates/iris-dev-core/Cargo.toml`
- [ ] T002 Create `crates/iris-dev-core/src/elicitation.rs` with empty module (stubs only — filled in Phase 2)
- [ ] T003 Create `crates/iris-dev-core/src/tools/scm.rs` with empty module (stubs only — filled in Phase 4)
- [ ] T004 Add `pub mod elicitation;` to `crates/iris-dev-core/src/lib.rs`
- [ ] T005 Add `pub mod scm;` to `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T006 `cargo build` — must compile clean

---

## Phase 2: Foundational — Elicitation Infrastructure

**Purpose**: `ElicitationStore` and `send_elicitation_or_fallback` helper used by all story phases.
**Independent test**: Unit tests for store create/lookup/expiry.

- [ ] T007 Write failing unit test `elicitation_state_expires`: insert a `PendingElicitation` with `expires_at = Instant::now()`, assert immediate lookup returns None — `crates/iris-dev-core/tests/unit/test_elicitation.rs`
- [ ] T008 Write failing unit test `elicitation_state_roundtrip`: insert with future expiry, lookup by id, assert all fields match — `crates/iris-dev-core/tests/unit/test_elicitation.rs`
- [ ] T009 Register `test_elicitation` test in `crates/iris-dev-core/Cargo.toml` `[[test]]` section
- [ ] T010 Implement `PendingElicitation` struct and `ElicitationAction` enum in `crates/iris-dev-core/src/elicitation.rs`
- [ ] T011 Implement `ElicitationStore`: `Arc<Mutex<HashMap<String, PendingElicitation>>>` with `insert(doc, action, content, ns) -> String` (returns UUID id), `lookup(id) -> Option<PendingElicitation>` (checks expiry), `clear(id)` — `crates/iris-dev-core/src/elicitation.rs`
- [ ] T012 Add `elicitation_store: Arc<ElicitationStore>` field to `IrisTools` struct; initialise in both `new()` and `with_registry()` — `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T013 Implement `write_open_hint(namespace: &str, document: &str)` free function: create `~/.iris-dev/` dir if needed, write `{"uri":"isfs://NS/doc","ts":epoch_ms}` to `~/.iris-dev/open-hint.json` — `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T014 **Phase gate**: `cargo test test_elicitation` — both unit tests pass

---

## Phase 3: US1 — iris_doc SCM Elicitation

**Story goal**: `iris_doc(mode=put)` on a locked document triggers elicitation in chat, not a popup.
**Independent test**: Put to namespace without SCM → success + `open_uri`. Put with `elicitation_answer=yes` + valid id → document written.

- [ ] T015 [US1] Write failing unit test `doc_params_elicitation_fields`: assert `IrisDocParams` deserializes `elicitation_id` and `elicitation_answer` fields correctly — `crates/iris-dev-core/tests/unit/test_doc_params.rs`
- [ ] T016 [US1] Write failing integration test `iris_doc_put_no_scm`: put a test class to USER namespace (no SCM), assert `success:true` and `open_uri` present in response — `crates/iris-dev-core/tests/integration/test_scm.rs`
- [ ] T017 [US1] Write failing integration test `iris_doc_put_elicitation_resume`: insert a `PendingElicitation` manually, call `iris_doc` with matching `elicitation_id` and `elicitation_answer=yes`, assert document written — `crates/iris-dev-core/tests/integration/test_scm.rs`
- [ ] T018 [US1] Register `test_scm` integration test in `crates/iris-dev-core/Cargo.toml`
- [ ] T019 [US1] Add `elicitation_answer: Option<String>` and `elicitation_id: Option<String>` to `IrisDocParams` in `crates/iris-dev-core/src/tools/doc.rs`
- [ ] T020 [US1] Update `handle_put` in `doc.rs`: if `elicitation_id` present → lookup store → skip `OnBeforeSave` → resume write directly
- [ ] T021 [US1] Update `handle_put` in `doc.rs`: call `OnBeforeSave` via xecute; parse response `action` value; if action=0 proceed; if action=1 create `PendingElicitation` and return elicitation fallback JSON; if action=7 (text prompt) create elicitation with `input_type=text`; if action=6 (alert) return SCM_REJECTED with alert message — `crates/iris-dev-core/src/tools/doc.rs`
- [ ] T021a [US1] Implement formal MCP `elicitation/create` path in `handle_put`: check if MCP client advertised `elicitation` capability during initialize (store flag in `IrisTools`); if yes, send raw JSON-RPC `{"method":"elicitation/create","params":{"message":"...","requestedSchema":{"type":"object","properties":{"confirm":{"type":"boolean"}},"required":["confirm"]}}}` instead of fallback JSON — `crates/iris-dev-core/src/tools/doc.rs` and `crates/iris-dev-core/src/elicitation.rs`
- [ ] T022 [US1] Call `write_open_hint(namespace, name)` after every successful `handle_put`; add `"open_uri": "isfs://NS/doc"` to success response — `crates/iris-dev-core/src/tools/doc.rs`
- [ ] T023 [US1] Remove `IRIS_SOURCE_CONTROL` and `IRIS_SKIP_SOURCE_CONTROL` env var branches from `handle_put` — `crates/iris-dev-core/src/tools/doc.rs`
- [ ] T024 [US1] **Phase gate**: `cargo test iris_doc_put_no_scm iris_doc_put_elicitation_resume doc_params_elicitation_fields` — all pass against live IRIS

---

## Phase 4: US2 — iris_compile Auto-Open

**Story goal**: Successful single-document compile writes sentinel and returns `open_uri`.
**Independent test**: Compile a single class → `open_uri` in response, sentinel file written.

- [ ] T025 [US2] Write failing integration test `iris_compile_open_uri`: compile `%Library.Base`, assert `open_uri` present in response and `~/.iris-dev/open-hint.json` was written/updated — `crates/iris-dev-core/tests/integration/test_scm.rs`
- [ ] T026 [US2] Update `iris_compile` tool: after successful compile of a single non-wildcard target, call `write_open_hint(namespace, target)` and add `"open_uri"` to response JSON — `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T027 [US2] Wildcard/batch compile (`target.contains('*')` or `targets_compiled > 1`): do NOT write hint and do NOT include `open_uri` — `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T028 [US2] **Phase gate**: `cargo test iris_compile_open_uri` passes; manual verify sentinel written

---

## Phase 5: US3 + US4 — iris_source_control Tool

**Story goal**: `iris_source_control` exposes status, menu, checkout, execute for any installed SCM.
**Independent test**: status on namespace without SCM returns `controlled:false` without error.

- [ ] T029 [P] [US3] Write failing unit test `scm_known_menu_items_nonempty`: assert `KNOWN_MENU_ITEMS` constant contains "CheckOut" — `crates/iris-dev-core/tests/unit/test_scm.rs`
- [ ] T030 [P] [US3] Write failing integration test `iris_source_control_status_uncontrolled`: call `iris_source_control(action=status, document=%Library.Base.cls)` in USER namespace with no SCM, assert `{"controlled":false,"editable":true}` — `crates/iris-dev-core/tests/integration/test_scm.rs`
- [ ] T031 [US3] Register `test_scm` unit test in `crates/iris-dev-core/Cargo.toml` (if not already from T018)
- [ ] T032 [US3] Implement `ScmParams`, `ScmActionEnum`, `ScmStatus`, `ScmMenuItem` structs in `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T033 [US3] Implement `KNOWN_MENU_ITEMS` constant: `["CheckOut","UndoCheckOut","CheckIn","GetLatest","Status","History","AddToSourceControl"]` — `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T034 [US3] Implement `action=status`: xecute `##class(%Studio.SourceControl.Base).%GetImplementationObject(doc)` — if null return `controlled:false`; else call `IsEditable`, parse `owner` — `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T035 [US3] Implement `action=menu`: for each item in `KNOWN_MENU_ITEMS`, xecute `OnMenuItem` call; collect items where `enabled=1` — `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T036 [US3] Implement `action=checkout`: xecute `UserAction(0,"CheckOut",doc)`; parse `action` response; if action=0 return success; if action=1 create `PendingElicitation(ScmExecute)` and return elicitation fallback — `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T037 [US4] Implement `action=execute`: xecute `UserAction(0, action_id, doc)`; parse `action`; if action=0 or 1 handle same as checkout; if action=7 (text prompt) create elicitation with `input_type=text`; on resume call `AfterUserAction` — `crates/iris-dev-core/src/tools/scm.rs`
- [ ] T038 [US3] Wire `iris_source_control` into `IrisTools` `#[tool_router]` impl with `#[tool(description="...")]` annotation — `crates/iris-dev-core/src/tools/mod.rs`
- [ ] T039 [US3] **Phase gate**: `cargo test scm_known_menu_items_nonempty iris_source_control_status_uncontrolled` — both pass

---

## Phase 6: VS Code Extension — FileSystemWatcher

**Story goal**: VS Code opens documents automatically when sentinel file is written.
**Independent test**: Manual — trigger `iris_doc(mode=put)` with ISFS workspace open, verify document opens.

- [ ] T040 [US2] In `vscode-iris-dev/src/extension.ts` `activate()`: create `~/.iris-dev/` directory if missing using Node.js `fs.mkdirSync` — `vscode-iris-dev/src/extension.ts`
- [ ] T041 [US2] Add `vscode.workspace.createFileSystemWatcher` on `~/.iris-dev/open-hint.json`; on `onDidChange`: read file, check `ts` within 3000ms, check ISFS workspace folder exists, call `vscode.window.showTextDocument(vscode.Uri.parse(hint.uri))` — `vscode-iris-dev/src/extension.ts`
- [ ] T042 [US2] Helper `hasIsfsWorkspace()`: returns true if any `vscode.workspace.workspaceFolders` has scheme `isfs` or `isfs-readonly` — `vscode-iris-dev/src/extension.ts`
- [ ] T043 [US2] Rebuild `.vsix`: `npm run package` in `vscode-iris-dev/` — output `vscode-iris-dev-0.2.0.vsix`
- [ ] T044 [US2] **Phase gate**: manual test with ISFS workspace — compile/put triggers auto-open

---

## Phase 7: Polish & Cleanup

- [ ] T045 [P] Remove `IRIS_SOURCE_CONTROL` and `IRIS_SKIP_SOURCE_CONTROL` from `crates/iris-dev-bin/src/cmd/mcp.rs` env var declarations
- [ ] T046 [P] Update `TROUBLESHOOTING.md` in objectscript-mcp repo: document new SCM elicitation behaviour and sentinel file
- [ ] T047 [P] Update `vscode-iris-dev` README: document `iris_source_control` tool and auto-open behaviour
- [ ] T048 Run full test suite `cargo test --all` — 0 failures
- [ ] T049 **Final gate**: `e2e_all_tools_respond` passes with new tool count (was 20, now 21 with `iris_source_control`)

---

## Dependencies

```
Phase 1 (Setup) → Phase 2 (Elicitation infra) → Phase 3 (US1 iris_doc)
                                               → Phase 4 (US2 iris_compile) [parallel with Phase 3]
Phase 2 done → Phase 5 (US3+US4 iris_source_control) [parallel with Phase 3+4]
Phase 3+4 done → Phase 6 (VS Code extension)
All phases → Phase 7 (Polish)
```

## Parallel Opportunities

- Phases 3, 4, 5 all start after Phase 2 — can run concurrently
- T029, T030 within Phase 5 are parallel (unit + integration test setup)
- T045, T046, T047 in Phase 7 are all parallel

## MVP

**Phases 1–3**: Elicitation infra + `iris_doc` SCM handling. This alone solves Nathan and Tim's popup problem and is a shippable increment.
