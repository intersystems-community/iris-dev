# Research: Better Docker Discovery Error Messages

**Feature**: 028-better-docker-discovery
**Date**: 2026-05-02
**Status**: Complete — all unknowns resolved through direct container investigation

## Decisions

| Decision | Choice | Rationale | Alternatives Considered |
|----------|--------|-----------|------------------------|
| Return type for `discover_via_docker_named()` | New `DiscoveryResult` enum (`Connected`, `NotFound`, `FoundUnhealthy(FailureMode)`) | Caller needs to distinguish "not found" (may continue cascade) from "found but unhealthy" (must stop cascade) — `Option<T>` cannot encode this | `Result<Option<T>>` — rejected: `Err` implies programming error, not expected failure mode; tuple `(Option<T>, Option<FailureMode>)` — rejected: less ergonomic |
| Return type for `discover_iris()` | New `IrisDiscovery` enum (`Found`, `NotFound`, `Explained`) | Callers need a third state meaning "message already emitted, emit nothing more" — current `Ok(None)` collapses "not found" and "explained" into the same value | Keep `Ok(None)`, set a thread-local flag — rejected: implicit, fragile; add a `bool` out-param — rejected: noisy at every call site |
| Cascade behavior when container found but unhealthy | Stop cascade, return `Explained` | User named a container explicitly; falling through to localhost:52773 risks silently connecting to the wrong IRIS | Continue cascade — rejected: leads to silent wrong-connection bug |
| Cascade behavior when container not found | Continue cascade | Container name may be a typo, container may not be running yet; localhost fallback is a reasonable recovery | Stop cascade — rejected: overly strict for a "not found" case |
| Localhost scan credentials | Use `IRIS_USERNAME`/`IRIS_PASSWORD` env vars (fall back to `_SYSTEM`/`SYS`) | User may have set non-default credentials; hardcoded `SYS` silently fails or connects to wrong instance | Keep hardcoded — rejected: inconsistent with how other discovery paths handle credentials |
| Enterprise image hint text | "Enterprise images do not include the private web server — use iris-community for local dev, or connect via IRIS_HOST+IRIS_WEB_PORT to an external Web Gateway" | Verified: CPF `WebServer=1` crashes the container (missing httpd binary, CSP.ini) | "Set WebServer=1 in CPF" — rejected: crashes enterprise containers with `<NOTOPEN>WebServer+38^STU1` |
| CI test scope | Community images without `#[ignore]`; enterprise images `#[ignore]` gated by `IRIS_LICENSE_KEY_PATH` | Community images cover all failure mode categories; enterprise adds the web-server-absent mode only | Require license key for all tests — rejected: blocks CI environments |
| `compile.rs` behavior on `Explained` | Exit code 1, no extra output | Discovery WARN already on stderr; duplicating the message adds noise | Print summary line — rejected: redundant |

## Root Cause: Enterprise Web Server Absence (Verified 2026-05-02)

Directly inspected running containers from `containers.intersystems.com`:

### Community (`iris-community:2026.1`, `irishealth-community:2026.1`)
- `iris.cpf` `[Startup]` section: `WebServer=1`, `WebServerPort=52773`
- `/usr/irissys/httpd/bin/httpd` — present (Apache binary, 2.9 MB)
- `/usr/irissys/csp/bin/CSP.ini` — present
- Result: private web server starts automatically on port 52773

### Enterprise (`iris:2026.1`, `irishealth:2026.1`)
- `iris.cpf` `[Startup]` section: `WebServer=0`, `WebServerPort=0`
- `/usr/irissys/httpd/` — **directory does not exist**
- `/usr/irissys/csp/bin/` — **directory does not exist**
- Result: IRIS starts only superserver (1972) and ISCAgent (2188)
- CPF merge `WebServer=1` **crashes** the container: `<NOTOPEN>WebServer+38^STU1`
  because the Apache binary infrastructure is absent

This is **intentional product design**: enterprise users deploy IRIS behind an external
Web Gateway (standalone Apache/nginx + ISC CSP Gateway module). The private web server is
a convenience bundled only in community editions.

## Affected Code Locations

| File | Location | Change needed |
|------|----------|---------------|
| `crates/iris-dev-core/src/iris/discovery.rs` | `discover_via_docker_named()` line 239 | Returns `DiscoveryResult` instead of `Option<IrisConnection>` |
| `crates/iris-dev-core/src/iris/discovery.rs` | `discover_iris()` line 95 | Returns `IrisDiscovery` instead of `Result<Option<IrisConnection>>`; localhost scan credentials; cascade-stop logic |
| `crates/iris-dev-core/src/iris/discovery.rs` | `probe_atelier_with_client()` line 20 | Surface container name/port in 401 WARN; return `FailureMode` signal |
| `crates/iris-dev-bin/src/cmd/mcp.rs` | line 85 | Pattern-match `IrisDiscovery`; suppress "No IRIS connection" WARN on `Explained` |
| `crates/iris-dev-bin/src/cmd/compile.rs` | line 52 | Pattern-match `IrisDiscovery`; exit 1 silently on `Explained` |
| `crates/iris-dev-core/tests/` | `docker_discovery_e2e.rs` (new) | 4-image regression harness |
| `crates/iris-dev-core/tests/unit/` | `test_discovery_unit.rs` (extend) or new | Unit tests for each failure mode |

## Existing Test Coverage Gap

`tests/discovery_tests.rs` has 3 tests — all use env var discovery or explicit connection.
Zero tests cover `discover_via_docker_named()` or the cascade-stop behavior. The unit tests
for modes (a)-(e) are entirely new.
