# Feature Specification: Better Docker Discovery Error Messages

**Feature Branch**: `028-better-docker-discovery`
**Created**: 2026-05-02
**Status**: Draft
**Closes**: GitHub issue #28

## Overview

When `IRIS_CONTAINER` is set, `discover_via_docker_named()` produces a single generic
"not found or not reachable via Docker" warning for three completely different failure modes.
Users cannot distinguish whether the container doesn't exist, exists but the web server
never started, or exists and is healthy but auth failed. The cascade then silently falls
through to a localhost port scan — potentially connecting to the *wrong* IRIS instance.

This feature replaces the single generic warning with tiered, actionable error messages
per failure mode, and stops the discovery cascade when `IRIS_CONTAINER` is set explicitly
and the named container is found but unhealthy.

### Background: Reproduced Failure Modes

Reproduced 2026-05-02 against four fresh containers (all from `containers.intersystems.com`):

| Image | Private web server | Result |
|-------|--------------------|--------|
| `iris-community:2026.1` | ✅ started | 401 (no `IRIS_PASSWORD`) |
| `iris:2026.1` (enterprise) | ❌ never started | TCP open, empty response |
| `irishealth-community:2026.1` | ✅ started | 401 (no `IRIS_PASSWORD`) |
| `irishealth:2026.1` (enterprise) | ❌ never started | TCP open, empty response |

### Root Cause of Enterprise Web Server Absence (Verified 2026-05-02)

The enterprise IRIS images (`iris:2026.1`, `irishealth:2026.1`) **do not ship the private
web server binary or CSP configuration**. This is a deliberate product split:

- Community images: include Apache httpd (`/usr/irissys/httpd/`), CSP Gateway
  (`/usr/irissys/csp/bin/CSP.ini`, `CSP.so`), and `WebServer=1` in `iris.cpf`
- Enterprise images: `WebServer=0` in `iris.cpf`, no `/usr/irissys/httpd/` directory,
  no `/usr/irissys/csp/bin/` — only superserver (1972) and ISCAgent (2188) start

Setting `WebServer=1` via CPF merge crashes the enterprise container with
`<NOTOPEN>WebServer+38^STU1` because the binary infrastructure is absent.

**The enterprise fix is an external Web Gateway** (standalone Apache/nginx with the ISC
CSP Gateway module), not a CPF setting. This is by design — enterprise users are expected
to deploy IRIS behind a proper web infrastructure, not the embedded private server.

**Implication for iris-dev error messages**: the hint for the enterprise web-server-absent
case must say "the enterprise IRIS image does not include the private web server — use an
external Web Gateway, or use `iris-community` / `irishealth-community` for local development.
Alternatively, connect via `IRIS_HOST`+`IRIS_WEB_PORT` pointing to an external Web Gateway."
It must NOT suggest `WebServer=1` in a CPF merge, which does not work.

Enterprise images (`iris:2026.1`, `irishealth:2026.1`) do not start the private web server
automatically — it requires a separate activation step. This is the root cause of the
original #28 report (build 161 containers).

---

## Clarifications

### Session 2026-05-02

- Q: Should the cascade stop entirely when `IRIS_CONTAINER` is set but probe fails? → A: Yes — if the user named a container explicitly, falling through to localhost:52773 risks silently connecting to a different IRIS instance. Emit a clear error and return `Ok(None)` without continuing to Steps 4-6.
- Q: Should docker exec be tried as a fallback when Atelier is unreachable? → A: Out of scope for this feature. Document in the error message that docker exec is available for `iris_execute` and `iris_test` when `IRIS_CONTAINER` is set. Full docker exec fallback for other tools is a separate feature.
- Q: When probe returns 401, the helpful 401-specific WARN fires before the generic WARN — should the generic WARN be suppressed? → A: Yes. When mode (d) is detected (401), only the specific WARN should fire. The generic WARN is redundant noise.
- Q: What counts as "container found" vs "container not found"? → A: "Container found" means bollard's `list_containers` returns an entry with a matching name. "Container not found" means no name match.
- Q: What is the actual root cause and fix when enterprise containers don't start the web server? → A: Verified 2026-05-02: enterprise images (`iris:2026.1`, `irishealth:2026.1`) do not ship the private web server binary (`/usr/irissys/httpd/`) or CSP configuration at all — by design. `WebServer=1` in a CPF merge crashes the container. The correct fix is an external Web Gateway. The error hint must NOT suggest CPF merge; must say to use `iris-community`/`irishealth-community` for local dev or connect via `IRIS_HOST`+`IRIS_WEB_PORT` to an external gateway.
- Q: Should the localhost scan (Step 4) use `IRIS_USERNAME`/`IRIS_PASSWORD` env vars or hardcoded `_SYSTEM`/`SYS`? → A: Use env var credentials. Hardcoded credentials in the localhost scan silently connect to the wrong IRIS or fail on instances with non-default credentials.
- Q: Should the tool-level `IRIS_UNREACHABLE` error also fire when discovery already emitted a specific actionable message? → A: No — suppress it. Return a third `discover_iris()` variant `Ok(Explained)` so callers know "already explained, don't add noise." User sees exactly one clear message.
- Q: Should the regression harness require a license key in CI? → A: No. Community tests run in CI (no key); enterprise tests are `#[ignore]`, gated by `IRIS_LICENSE_KEY_PATH` env var.
- Q: For `iris-dev compile` CLI subcommand, when discovery returns `Explained`, what should happen? → A: Exit with code 1, no extra output — the discovery WARN already appeared on stderr. Callers pattern-match `IrisDiscovery` explicitly rather than using `?`+`.context()`.

---

## User Scenarios & Testing

### User Story 1 — Container not found in Docker API (Priority: P1)

A developer sets `IRIS_CONTAINER=my-iris` but the container name is wrong or the container
hasn't started yet.

**Independent Test**: Set `IRIS_CONTAINER=nonexistent-container`, run any tool. Error message
should clearly say the container wasn't found.

**Acceptance Scenarios**:

1. **Given** `IRIS_CONTAINER=nonexistent-iris`, **When** any tool is called, **Then** stderr
   contains `"Container 'nonexistent-iris' not found in Docker"` (not "not reachable").
2. **Given** the above, **Then** the discovery cascade does NOT continue to localhost scan
   (no connections attempted to `localhost:52773`).

---

### User Story 2 — Container found, port mapped, web server not running (Priority: P1)

A developer uses an enterprise IRIS image (`iris:2026.1` or `irishealth:2026.1`) which does
not start the private web server automatically. The port is mapped, TCP connects, but the
response is empty or connection-refused.

**Independent Test**: Start `containers.intersystems.com/intersystems/iris:2026.1` with
`-p 52791:52773`. Run `IRIS_CONTAINER=repro-enterprise-2026 iris-dev mcp`. Error message
should mention the web server.

**Acceptance Scenarios**:

1. **Given** a container with port 52773 mapped but web server not running, **When** any
   tool is called, **Then** stderr contains all of:
   - Container name and the host:port that was probed
   - "Atelier REST API is not responding"
   - Hint: "Enterprise IRIS images (iris:, irishealth:) do not include the private web server — use iris-community or irishealth-community for local dev, or connect via IRIS_HOST+IRIS_WEB_PORT to an external Web Gateway"
   - Note that `iris_execute` and `iris_test` still work via docker exec
   - **Must NOT** suggest setting `WebServer=1` in CPF (verified: crashes enterprise containers with `<NOTOPEN>WebServer+38^STU1`)
2. **Given** the above, **Then** the discovery cascade does NOT continue to localhost scan.
3. **Given** the above, **Then** the error notes that docker exec tools (`iris_execute`,
   `iris_test`) are still available when `IRIS_CONTAINER` is set.

---

### User Story 3 — Container found, port not mapped (Priority: P1)

A developer starts a container without exposing port 52773 (common when using
`iris-devtester` default factory methods, which don't map the web port).

**Independent Test**: Start a container without `-p 52773:...`. Set `IRIS_CONTAINER` to its
name. Run any tool. Error should say port is not mapped.

**Acceptance Scenarios**:

1. **Given** a running container with port 52773 NOT mapped to a host port, **When** any
   tool is called, **Then** stderr contains:
   - Container name
   - "port 52773 is not mapped to a host port"
   - Hint: "Restart with `-p <host_port>:52773` or use `IRIS_HOST`+`IRIS_WEB_PORT` instead"
2. **Given** the above, **Then** cascade does NOT continue.

---

### User Story 4 — Container found, port mapped, web server returns 401 (Priority: P1)

A developer starts an IRIS container without `IRIS_PASSWORD`, so `_SYSTEM` has only OS
authentication and basic auth is rejected.

**Independent Test**: Start `iris-community:2026.1` without `-e IRIS_PASSWORD=SYS`.
Set `IRIS_CONTAINER`. Run any tool. Should see exactly one actionable warning — not two.

**Acceptance Scenarios**:

1. **Given** a container returning 401 on Atelier probe, **When** any tool is called,
   **Then** stderr contains exactly one warning (not two) with: container name, port probed,
   hint about `IRIS_PASSWORD`, and the `docker run -e IRIS_PASSWORD=SYS` fix.
2. **Given** the above, **Then** the generic "not found or not reachable" message does NOT
   also appear.
3. **Given** the above, **Then** cascade does NOT continue.

---

### User Story 5 — Regression harness: all four image types pass (Priority: P1)

A CI test suite spins up all four container types fresh and verifies the correct error
message is emitted for each.

**Independent Test**: `cargo test --test docker_discovery_e2e -- --ignored`

**Acceptance Scenarios**:

1. **Given** all four containers (`iris-community:2026.1`, `iris:2026.1`,
   `irishealth-community:2026.1`, `irishealth:2026.1`) running from `containers.intersystems.com`,
   **When** the e2e test runs, **Then** each produces the expected failure mode classification
   and message.
2. **Given** any future IRIS image release, **When** the harness is run against it,
   **Then** the test documents which failure mode it hits (not silently passes with wrong behavior).

---

### Edge Cases

- Container found, port mapped, probe returns non-401 HTTP error (e.g. 503): treat as mode (c) — "web server not responding correctly", include the actual HTTP status code in the message.
- `IRIS_CONTAINER` set to empty string: existing behavior (skip) is correct — no change.
- `IRIS_CONTAINER` set but Docker daemon not reachable: emit "Could not connect to Docker daemon — is Docker running?" rather than the generic message.
- Multiple containers with similar names: `discover_via_docker_named` matches exact name, so this is not an issue here (it's `discover_via_docker`'s problem).

---

## Requirements

### Functional Requirements

- **FR-001**: `discover_via_docker_named()` MUST distinguish and emit separate log messages for:
  - Mode (a): Docker daemon unreachable → `"Could not connect to Docker daemon"`
  - Mode (b): Container not found → `"Container '{name}' not found in Docker (is it running?)"`
  - Mode (c): Container found, port not mapped → `"Container '{name}' found but port 52773 is not mapped to a host port — restart with -p <port>:52773"`
  - Mode (d): Container found, port mapped, probe fails (web server absent/down) → `"Container '{name}' found at localhost:{port} but Atelier REST API is not responding. Enterprise IRIS images (iris:, irishealth:) do not include the private web server — use iris-community or irishealth-community for local dev, or connect via IRIS_HOST+IRIS_WEB_PORT pointing to an external Web Gateway. Community images: restart with -e IRIS_PASSWORD=SYS."`
  - Mode (e): Container found, port mapped, probe returns 401 → existing helpful message (already correct); suppress the second generic WARN

- **FR-002**: When `IRIS_CONTAINER` is set and the named container is **found** (modes c, d, e), the discovery cascade MUST NOT continue to Steps 4-6 (localhost scan, generic Docker scan, VS Code settings). Return `IrisDiscovery::Explained` after emitting the mode-specific message.

- **FR-003**: When `IRIS_CONTAINER` is set and the container is **not found** (mode b), the cascade MAY continue (the name may be a typo, or the container may not be running yet — falling through to localhost may be a reasonable recovery).

- **FR-004**: All mode-specific messages for modes (c) and (d) MUST include a note that `iris_execute` and `iris_test` remain available via docker exec when `IRIS_CONTAINER` is set.

- **FR-005**: The existing 401-specific WARN (line 39-44 in `probe_atelier_with_client`) MUST remain. The generic WARN (line 155-158) MUST be suppressed for mode (e) — replaced with a mode-specific variant that includes container name and port.

- **FR-006**: `discover_via_docker_named()` MUST return a `DiscoveryResult` enum (or equivalent structured type) rather than `Option<IrisConnection>`, so the caller can distinguish "not found" from "found but unhealthy" and apply FR-002/FR-003 correctly.
- **FR-007**: The localhost port scan (Step 4 in `discover_iris()`) MUST use `IRIS_USERNAME` and `IRIS_PASSWORD` env vars (falling back to `_SYSTEM`/`SYS` if unset) rather than the currently hardcoded `_SYSTEM`/`SYS` credentials.
- **FR-008**: `discover_iris()` MUST return `IrisDiscovery` (a new enum) instead of `Result<Option<IrisConnection>>`. The `Explained` variant signals that a specific actionable message was already emitted. Callers MUST pattern-match explicitly — NOT use `?` + `.context()`. Behavior per caller:
  - `mcp.rs`: receive `Explained` → skip the "No IRIS connection — tools return IRIS_UNREACHABLE" warn, proceed with `conn = None` (tools still work for non-IRIS tools like `skill`, `kb`)
  - `compile.rs`: receive `Explained` → exit with code 1, no additional output (discovery WARN already on stderr)
  - Tool handlers (`get_iris()`): unchanged — `Explained` never reaches them because discovery runs once at startup; `IrisTools` holds `Option<IrisConnection>` which is `None` when `Explained`
- **FR-009**: The regression E2E test suite MUST be split: community-image tests run without `#[ignore]` (no license key required); enterprise-image tests MUST be `#[ignore]` and gated by `IRIS_LICENSE_KEY_PATH` env var.

### Key Entities

- **DiscoveryResult**: Replaces `Option<IrisConnection>` return from `discover_via_docker_named()`. Variants: `Connected(IrisConnection)`, `NotFound`, `FoundUnhealthy(FailureMode)`.
- **FailureMode**: Enum: `PortNotMapped`, `AtelierNotResponding { port: u16 }`, `AtelierAuth401 { port: u16 }`, `AtelierHttpError { port: u16, status: u16 }`.
- **IrisDiscovery**: Replaces `Result<Option<IrisConnection>>` return from `discover_iris()`. Variants: `Found(IrisConnection)`, `NotFound`, `Explained` — where `Explained` means a specific actionable message was already emitted to the user and no further error should be surfaced by callers (tools, MCP server startup).

---

## Success Criteria

- **SC-E2E**: E2E tests against all four `containers.intersystems.com` image types pass,
  each producing the correct failure mode classification.
- **SC-001**: Mode (a)-(e) each produce distinct, actionable log messages — verified by unit tests mocking each condition.
- **SC-002**: `IRIS_CONTAINER` set + container found (modes c/d/e) → cascade stops, no localhost:52773 connection attempted — verified by unit test.
- **SC-003**: `IRIS_CONTAINER` set + container not found (mode b) → cascade continues to localhost scan — verified by unit test.
- **SC-004**: Mode (e) 401 → exactly one WARN emitted, not two — verified by unit test checking log output.
- **SC-005**: All existing `test_toolset`, `test_compile_params`, and other unit tests continue to pass (no regressions).

---

## Assumptions

- The enterprise image web server absence is not a bug in the image — it's by design. The error message should not say "this is a bug" but rather give the user the correct fix.
- `discover_via_docker_named()` is the only place this fix is needed — `discover_via_docker()` (the generic scan) uses a different code path and its behavior is out of scope.
- The `containers.intersystems.com` images for `2026.1` are representative of the failure modes. Future versions may behave differently but the tiered error messages will remain correct.
