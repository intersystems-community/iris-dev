# Implementation Plan: Better Docker Discovery Error Messages

**Branch**: `028-better-docker-discovery` | **Date**: 2026-05-02 | **Spec**: `specs/028-better-docker-discovery/spec.md`
**Input**: Feature specification from `/specs/028-better-docker-discovery/spec.md`

## Summary

`discover_via_docker_named()` currently returns `Option<IrisConnection>`, collapsing five
distinct failure modes into one generic "not found or not reachable" warning. This feature
introduces two new enums — `DiscoveryResult` (from the named lookup) and `IrisDiscovery`
(from the full cascade) — that carry structured failure information. The caller emits
mode-specific, actionable messages and stops the cascade when a named container is found
but unhealthy, preventing silent fallthrough to the wrong IRIS. Root cause of the enterprise
web server absence was verified directly: enterprise images don't ship the httpd binary or
CSP config by design; CPF merge `WebServer=1` crashes them.

## Technical Context

**Language/Version**: Rust 1.92 (`crates/iris-dev-core`, `crates/iris-dev-bin`)
**Primary Dependencies**: `bollard` (already workspace); `reqwest` (already workspace). No new deps.
**Storage**: N/A — discovery is stateless
**Testing**: `cargo test` — unit tests (mock bollard/HTTP responses) + E2E tests (`#[ignore]` for enterprise, live Docker for community)
**Target Platform**: macOS arm64/x86_64, Linux x86_64, Windows x86_64 (zero-install binary)
**Project Type**: Single Rust workspace — changes span `iris-dev-core` (library) and `iris-dev-bin` (binary)
**Performance Goals**: Discovery latency unchanged — new enum return types add no overhead
**Constraints**: No new crate dependencies (Constitution VII); `discover_iris()` must remain infallible from caller perspective (no `Result` in new signature)
**Scale/Scope**: 2 affected binary call sites (`mcp.rs`, `compile.rs`), 1 core module (`discovery.rs`), 2 test files (new + extended)

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Install Binary | ✅ PASS | No new crates, no new install steps |
| II. ObjectScript Sanity | ✅ N/A | No new ObjectScript API calls — pure Rust enum refactor |
| III. HTTP-First Execution | ✅ PASS | No new Docker-required tools; existing tool behavior unchanged |
| IV. Test-First, Fixture-Driven | ✅ PASS — gated | Unit tests for all 5 failure modes written first; E2E tests written before implementation |
| V. Output Shape Parity | ✅ N/A | No tool response shape changes; this is internal discovery infrastructure |
| VI. Environment Guard | ✅ N/A | No new write-capable tools |
| VII. Dependency Minimalism | ✅ PASS | Zero new crate dependencies |

*All gates pass. No violations.*

## Project Structure

### Documentation

```text
specs/028-better-docker-discovery/
├── plan.md          # This file
├── research.md      # Root cause findings, decision table
├── data-model.md    # DiscoveryResult, FailureMode, IrisDiscovery enums + message templates
├── quickstart.md    # How to run the regression harness
├── contracts/
│   └── discovery-api.md   # Signature changes, caller migration patterns
└── tasks.md         # Phase 2 output (/speckit.tasks)
```

### Source Code

```text
crates/iris-dev-core/src/iris/
└── discovery.rs         # Core changes: new enums + new return types + tiered messages

crates/iris-dev-bin/src/cmd/
├── mcp.rs               # Caller: handle IrisDiscovery::{Found,NotFound,Explained}
└── compile.rs           # Caller: handle IrisDiscovery::{Found,NotFound,Explained}

crates/iris-dev-core/tests/
├── unit/
│   └── test_discovery_unit.rs   # NEW — unit tests for all 5 failure modes (mock Docker)
└── docker_discovery_e2e.rs      # NEW — regression harness: 4 image types
```

**Structure Decision**: Single-crate modification. All changes in `iris-dev-core/src/iris/discovery.rs`
plus caller updates in the binary crate. No new modules, no new files in `src/` — only the
two new enum types added to `discovery.rs` alongside the existing functions.

## Complexity Tracking

One non-obvious design decision worth tracking:

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| `IrisDiscovery` as a top-level enum replacing `Result<Option<T>>` | `mcp.rs` and `compile.rs` need to distinguish three states: connected / not found / already explained — `Option<T>` can only encode two states | Thread-local flag `DISCOVERY_EXPLAINED: AtomicBool` — rejected: implicit, hard to test, invisible to callers |
| `discover_iris()` is now infallible (no `Result`) | Errors during discovery (HTTP client build failure, etc.) are non-fatal — they should log and return `NotFound`, not propagate as `Err` that crashes the MCP server startup | Keep `Result` + add `Explained` variant to error type — rejected: `anyhow::Error` doesn't compose with `Explained` cleanly |
