# Tasks: 017 — Interop Tool Group

**Branch**: `017-interop-tools` on `devx/iris-dev`
**Deadline**: AIML75 demo, April 27
**MVP**: 9 tools (6 lifecycle + 3 observability)
**TDD**: ALL tests written first, confirmed FAILING, gating every phase

---

## Phase 0: Write ALL Tests (Red Gate)

**Purpose**: Write every unit and e2e test before any implementation. Confirm all FAIL. This is the gate — no implementation begins until Phase 0 is complete and all tests are confirmed red.

- [ ] T001 Create `crates/iris-dev-core/tests/interop_unit_tests.rs` with unit tests for all 9 tools: mock `IrisConnection` xecute/query responses; verify param parsing, response shaping, error codes. Include tests for:
  - `interop_production_status`: parses JSON state, handles full_status flag
  - `interop_production_start`: returns Running state on success
  - `interop_production_stop`: timeout + force params passed correctly
  - `interop_production_update`: success response shape
  - `interop_production_needs_update`: returns boolean
  - `interop_production_recover`: success response shape
  - `interop_logs`: filters by log_type, respects limit
  - `interop_queues`: returns array of {name, count}
  - `interop_message_search`: filters by source/target/class
  - All 9: IRIS_UNREACHABLE when no connection
  - All 9: structured error on IRIS error response

- [ ] T002 Create `crates/iris-dev-core/tests/interop_e2e_tests.rs` with e2e tests: spawn `iris-dev mcp`, send MCP tool calls. Include tests for:
  - `interop_production_status` returns valid JSON against iris-dev-iris
  - `interop_logs` returns structured log entries
  - `interop_queues` returns queue array
  - `tools/list` returns 32 tools (23 existing + 9 new)
  - No tool names contain dots
  - All tools return structured `{success, error_code}` on error

- [ ] T003 Run `cargo test --test interop_unit_tests --test interop_e2e_tests` — confirm ALL tests FAIL

**Gate 0 checkpoint**: Every test is red. No implementation code exists yet.

---

## Phase 1: Implement 6 Lifecycle Tools

**Purpose**: Port the 6 production lifecycle tools from team-23's Python reference. All use `IrisConnection.xecute()` with inline ObjectScript.

- [ ] T004 Create `crates/iris-dev-core/src/tools/interop.rs` with param structs for all 9 tools (serde Deserialize + schemars JsonSchema)
- [ ] T005 Implement `interop_production_status` in `interop.rs`: xecute inline ObjectScript that calls `Ens.Director.GetProductionStatus()` and Writes JSON with production name + state string + state code; with `full_status`, iterate `Ens.Config.Item` for per-component breakdown
- [ ] T006 [P] Implement `interop_production_start`: xecute `Ens.Director.StartProduction(name)`
- [ ] T007 [P] Implement `interop_production_stop`: xecute `Ens.Director.StopProduction(timeout, force)`
- [ ] T008 [P] Implement `interop_production_update`: xecute `Ens.Director.UpdateProduction(timeout, force)`
- [ ] T009 [P] Implement `interop_production_needs_update`: xecute `Ens.Director.ProductionNeedsUpdate()` → boolean
- [ ] T010 [P] Implement `interop_production_recover`: xecute `Ens.Director.RecoverProduction()`
- [ ] T011 Register all 6 lifecycle tools in `src/tools/mod.rs` via `#[tool_router]`
- [ ] T012 Run `cargo test --test interop_unit_tests` — lifecycle tool tests must PASS

**Gate 1 checkpoint**: 6 lifecycle unit tests green. `tools/list` returns 29 tools.

---

## Phase 2: Implement 3 Observability Tools

**Purpose**: Port the 3 observability tools. All use `IrisConnection.query()` with SQL.

- [ ] T013 Implement `interop_logs`: query `SELECT ID, TimeLogged, Type, ConfigName, Text FROM Ens_Util.Log WHERE ...` with filters for item_name, log_type, limit
- [ ] T014 [P] Implement `interop_queues`: query `SELECT * FROM Ens.Queue_Enumerate()`
- [ ] T015 [P] Implement `interop_message_search`: query `SELECT ID, TimeCreated, SourceConfigName, TargetConfigName, MessageBodyClassName, Status FROM Ens.MessageHeader WHERE ...` with source/target/class filters
- [ ] T016 Register all 3 observability tools in `src/tools/mod.rs`
- [ ] T017 Run `cargo test --test interop_unit_tests` — ALL 9 tool unit tests must PASS
- [ ] T018 Run `cargo test --test interop_e2e_tests` — e2e tests must PASS against iris-dev-iris (requires Ensemble enabled)

**Gate 2 checkpoint**: ALL unit tests green. ALL e2e tests green. `tools/list` = 32 tools. 9 interop tools return structured JSON.

---

## Phase 3: Demo Rehearsal

**Purpose**: Full demo flow against iris-dev-iris. Recordable, high-performance output.

- [ ] T019 Create or verify an Ensemble-enabled namespace in iris-dev-iris container
- [ ] T020 Write a simple test production class (`Demo.TestProduction` with one File passthrough service) and compile into iris-dev-iris
- [ ] T021 Run full demo flow: status → start → check logs → check queues → stop → verify status=Stopped → start → recover
- [ ] T022 Verify all tool responses are structured JSON suitable for programmatic capture
- [ ] T023 Time each tool response — verify all complete in <2s
- [ ] T024 Record the demo flow as a script in `specs/017-interop-tools/demo-script.md`

**Gate 3 checkpoint**: Working recordable demo. All responses structured. Sub-2s latency.

---

## Dependencies

- Phase 0 blocks ALL implementation (TDD gate)
- Phase 1 blocks Phase 2 (lifecycle tools must register before observability)
- Phase 2 blocks Phase 3 (all 9 tools needed for demo)
- Phases 1-2: T006-T010 and T014-T015 are parallelizable within their phases

## Implementation Strategy

1. Phase 0 first — write ALL tests, confirm red
2. Phase 1 — lifecycle tools, one at a time, unit test gate after each
3. Phase 2 — observability tools, unit + e2e gates
4. Phase 3 — demo rehearsal before April 27

Total: 24 tasks across 4 phases.
