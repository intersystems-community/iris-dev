# Implementation Plan: iris-dev v2.1 — SCM Elicitation, Auto-Open, SCM Tools

**Branch**: `020-scm-elicitation-auto-open` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)

## Summary

Add MCP-spec-compliant Elicitation to `iris_doc(mode=put)` so source control locks trigger a chat question instead of a popup. Add `iris_source_control` tool for lock status, menu, checkout, and arbitrary SCM actions. Add sentinel-file-based auto-open so VS Code opens documents after write. Extends the 020 branch on top of the v2 unified server.

---

## Technical Context

**Language/Version**: Rust 2021, tokio async, rmcp 1.2
**New dependencies**: `uuid` crate (elicitation IDs), `dirs` crate (home dir for sentinel file)
**Elicitation**: MCP spec `elicitation/create` via raw JSON-RPC; structured JSON fallback
**IRIS hooks**: `%Studio.SourceControl.Base` via Atelier xecute
**Sentinel file**: `~/.iris-dev/open-hint.json`
**Extension**: vscode-iris-dev `FileSystemWatcher` on sentinel file

---

## Phase 1: Elicitation Infrastructure

**Goal**: Add `PendingElicitation` state to `IrisTools` and ability to send `elicitation/create` over the MCP connection.

### Tasks
1. **[TEST]** Write unit test `elicitation_state_expires`: create a `PendingElicitation`, advance time past 5 minutes, assert lookup returns None — `crates/iris-dev-core/tests/unit/test_elicitation.rs`
2. **[TEST]** Write unit test `elicitation_state_roundtrip`: create, lookup by id, assert fields match — same file
3. Add `uuid` and `dirs` to `crates/iris-dev-core/Cargo.toml`
4. Create `crates/iris-dev-core/src/elicitation.rs` with `PendingElicitation`, `ElicitationStore` (Arc<Mutex<HashMap>>), insert/lookup/expire methods
5. Add `elicitation_store: Arc<ElicitationStore>` field to `IrisTools`; initialise in `new()` and `with_registry()`
6. Add helper `send_elicitation_or_fallback(ctx, id, message, schema) -> CallToolResult` — tries `elicitation/create` JSON-RPC; falls back to structured JSON response
7. **Phase gate**: `cargo test test_elicitation` passes

---

## Phase 2: iris_doc SCM Elicitation

**Goal**: `iris_doc(mode=put)` calls `OnBeforeSave`, handles elicitation, resumes on answer.

### Tasks
1. **[TEST]** Write integration test `iris_doc_put_no_scm`: put to namespace without SCM, assert success + `open_uri` in response — `crates/iris-dev-core/tests/integration/test_scm.rs`
2. **[TEST]** Write integration test `iris_doc_put_scm_bypass`: put with `elicitation_answer="yes"` and valid `elicitation_id`, assert document written — same file
3. Update `IrisDocParams` in `doc.rs`: add `elicitation_answer: Option<String>`, `elicitation_id: Option<String>`
4. Update `handle_put` in `doc.rs`:
   - If `elicitation_id` present: look up `PendingElicitation`, resume write directly (skip OnBeforeSave)
   - Otherwise: call `OnBeforeSave` via xecute; parse response
   - If action=0: proceed with write
   - If action=1 (dialog): create `PendingElicitation`, call `send_elicitation_or_fallback`, return without writing
   - If error status: return `SCM_REJECTED`
5. Add `write_open_hint(namespace, document)` function — writes sentinel JSON to `~/.iris-dev/open-hint.json`
6. Call `write_open_hint` after every successful `handle_put`
7. Add `open_uri` field to successful put response
8. **Phase gate**: `cargo test iris_doc_put_no_scm iris_doc_put_scm_bypass` pass

---

## Phase 3: iris_compile open_uri

**Goal**: Single-document compile writes sentinel and returns `open_uri`.

### Tasks
1. After successful compile of a single non-wildcard target, call `write_open_hint` and add `open_uri` to response JSON
2. Wildcard/batch compiles do NOT write hint (no single document to open)
3. **Phase gate**: manual test — compile a single class, verify `open_uri` in response and `~/.iris-dev/open-hint.json` written

---

## Phase 4: iris_source_control tool

**Goal**: New multi-action tool for SCM status, menu, checkout, execute.

### Tasks
1. **[TEST]** Write unit test `scm_action_known_names`: assert the known menu item names list is non-empty and contains "CheckOut" — `crates/iris-dev-core/tests/unit/test_scm.rs`
2. **[TEST]** Write integration test `iris_source_control_status_uncontrolled`: call status on namespace without SCM, assert `controlled:false` — `crates/iris-dev-core/tests/integration/test_scm.rs`
3. Create `crates/iris-dev-core/src/tools/scm.rs` with `ScmParams`, `ScmAction` enum, `handle_iris_source_control`
4. Implement `action=status`: xecute `%Studio.SourceControl.Base.%GetImplementationObject` + `IsEditable`; parse result
5. Implement `action=menu`: call `OnMenuItem` for each known menu item name via xecute; return enabled items
6. Implement `action=checkout`: call `UserAction(0, "CheckOut", document)`; parse Action response; elicitation if action=1
7. Implement `action=execute`: call `UserAction(0, action_id, document)`; parse Action response; elicitation if action=1 or 7; call `AfterUserAction` with answer on resume
8. Wire `iris_source_control` into `IrisTools` tool router in `mod.rs`
9. **Phase gate**: `cargo test iris_source_control_status_uncontrolled` passes; manual test of menu/checkout on SCM-enabled namespace

---

## Phase 5: VS Code Extension — FileSystemWatcher

**Goal**: vscode-iris-dev opens documents automatically when sentinel file is written.

### Tasks
1. Add `dirs` npm package to `vscode-iris-dev/package.json` (or use `os.homedir()`)
2. In `extension.ts` `activate()`: set up `vscode.workspace.createFileSystemWatcher` on `~/.iris-dev/open-hint.json`
3. On file change: read JSON, check `ts` is within 3 seconds, check ISFS workspace folder exists, call `vscode.window.showTextDocument(vscode.Uri.parse(hint.uri))`
4. Create `~/.iris-dev/` directory on first write (in binary) and first watch setup (in extension)
5. Rebuild `.vsix`: `npm run package` in `vscode-iris-dev/`
6. **Phase gate**: manual test — `iris_doc(mode=put)` on class in ISFS workspace → document opens in VS Code automatically

---

## Phase 6: Cleanup + Tests

### Tasks
1. Remove `IRIS_SOURCE_CONTROL` and `IRIS_SKIP_SOURCE_CONTROL` env var handling from `doc.rs` and `mcp.rs`
2. Update `TROUBLESHOOTING.md` and extension README to document new SCM behaviour
3. Run full test suite: `cargo test --all`
4. **Final gate**: all tests pass; `iris_source_control` and elicitation flow demonstrated end-to-end
