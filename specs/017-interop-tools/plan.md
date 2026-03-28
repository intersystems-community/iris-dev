# Implementation Plan: 017 — Interop Tool Group

**Branch**: `017-interop-tools` | **Date**: 2026-03-22 | **Spec**: [spec.md](spec.md)
**Deadline**: AIML75 demo, April 27

## Summary

Add 9 interop_* MCP tools (MVP) to iris-dev exposing IRIS Interoperability productions via Atelier REST. Port from team-23's Python reference implementation. All tools follow the existing `IrisConnection.xecute()` + `IrisConnection.query()` pattern.

## Technical Context

**Language**: Rust 2021 (existing iris-dev workspace)
**Dependencies**: No new crates — uses existing `rmcp`, `reqwest`, `serde`, `schemars`
**IRIS APIs**: Atelier REST xecute + query (no native protocol)
**Testing**: `cargo test` unit + integration against `iris-dev-iris` (localhost:52780)
**Reference**: `~/ws/team-23/src/mcp_server_iris/interoperability.py` (510 lines, 18 functions)

## TDD Gate Structure

**Critical requirement from user**: Comprehensive unit and e2e tests written FIRST, confirmed FAILING, then implementation. Full test passage gates each development phase.

### Gate 1: Unit tests (no IRIS)
- Mock `IrisConnection` responses
- Verify each tool's param parsing, response shaping, error handling
- Must FAIL before implementation, PASS after

### Gate 2: E2e tests (live iris-dev-iris)
- Spawn `iris-dev mcp`, send MCP tool calls
- Verify real IRIS Ensemble calls return structured data
- Must FAIL before implementation, PASS after

### Gate 3: Integration count
- `tools/list` returns 32 tools (23 existing + 9 new)
- No dots in tool names
- All return structured JSON `{success, error_code}`

## Project Structure

All changes in the existing `iris-dev` workspace:

```
crates/iris-dev-core/
  src/tools/
    mod.rs                    ← UPDATE: add interop tools to #[tool_router]
    interop.rs                ← NEW: 9 interop_* tool implementations
  tests/
    interop_unit_tests.rs     ← NEW: unit tests with mocked IRIS responses
    interop_e2e_tests.rs      ← NEW: e2e tests against iris-dev-iris
```

No new crates. No new directories beyond `interop.rs` and its tests.

## Research Findings

### ByRef parameter handling
`Ens.Director.GetProductionStatus` uses ByRef params. Atelier xecute doesn't support ByRef return directly. Solution: inline ObjectScript that calls the method and Writes JSON output.

Pattern:
```objectscript
Set sc=##class(Ens.Director).GetProductionStatus(.n,.s)
Write {"production":(n),"state":($Case(s,1:"Running",2:"Stopped",3:"Suspended",4:"Troubled",:"Unknown"))}
```

### SQL for observability tools
- Logs: `SELECT * FROM Ens_Util.Log WHERE ...`
- Queues: `SELECT * FROM Ens.Queue_Enumerate()`
- Messages: `SELECT * FROM Ens.MessageHeader WHERE ...`

All use existing `IrisConnection.query()`.

## Phase Plan

### Phase 0: Write ALL tests (TDD red phase)
Write every unit and e2e test. Confirm they all FAIL. This is the gate.

### Phase 1: Implement 6 lifecycle tools
Implement `interop_production_status`, `start`, `stop`, `update`, `needs_update`, `recover`.
**Gate**: unit tests for lifecycle tools PASS; `tools/list` returns 29 tools.

### Phase 2: Implement 3 observability tools
Implement `interop_logs`, `interop_queues`, `interop_message_search`.
**Gate**: ALL unit tests PASS; e2e tests PASS; `tools/list` returns 32 tools.

### Phase 3: Demo rehearsal
Full demo flow against iris-dev-iris with real production.
**Gate**: recordable demo works end-to-end.

## Complexity Tracking

No new patterns. No new dependencies. Purely mechanical port of team-23's interoperability.py to Rust using existing Atelier REST infrastructure. The only complexity is ByRef parameter handling (solved with inline ObjectScript).
