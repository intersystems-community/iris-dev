# Tasks: Better Docker Discovery Error Messages

**Input**: Design documents from `/specs/028-better-docker-discovery/`
**Repo**: `~/ws/iris-dev` (Rust — `crates/iris-dev-core` + `crates/iris-dev-bin`)
**Constitution**: Principle IV — unit tests (no Docker needed) before implementation; E2E tests `#[ignore]` for enterprise (need `IRIS_LICENSE_KEY_PATH`), no `#[ignore]` for community.
**NOTE**: `#[ignore]` enterprise E2E tests are MANDATORY phase gates — they cannot be skipped, only run manually. A phase is not complete until its E2E gate passes, even if that requires a human with a license key to run it.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add new enum types and register the new test binary — no behavior changes yet.

- [x] T001 Add `DiscoveryResult` and `FailureMode` enums to `crates/iris-dev-core/src/iris/discovery.rs` — stub only, no methods, after existing `use` imports
- [x] T002 Add `IrisDiscovery` enum to `crates/iris-dev-core/src/iris/discovery.rs` — stub only, after `DiscoveryResult`
- [x] T003 Add `[[test]]` entry for `docker_discovery_e2e` in `crates/iris-dev-core/Cargo.toml` — path `tests/docker_discovery_e2e.rs`; create empty stub file
- [x] T004 Add `[[test]]` entry for `test_discovery_unit` in `crates/iris-dev-core/Cargo.toml` — path `tests/unit/test_discovery_unit.rs`; create empty stub file
- [x] T005 Verify `cargo check -p iris-dev-core` passes with new enum stubs

**Checkpoint**: `cargo check` passes. New enums compile. Test stubs present.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Refactor `discover_via_docker_named()` to return `DiscoveryResult` and `discover_iris()` to return `IrisDiscovery`. All existing tests must still pass. No message changes yet — just the return type plumbing.

### Tests for Phase 2 (write first — must FAIL before implementation)

- [x] T006 [P] Write unit test: `discover_iris()` with no env vars and no Docker containers returns `IrisDiscovery::NotFound` in `crates/iris-dev-core/tests/unit/test_discovery_unit.rs` — the `IrisDiscovery` enum exists (from T002) but `discover_iris()` still returns `Result<Option<IrisConnection>>`; the test will FAIL TO COMPILE due to type mismatch — that compile failure IS the valid RED state (WRITE FIRST)
- [x] T007 [P] Update `crates/iris-dev-core/tests/discovery_tests.rs` — add `use iris_dev_core::iris::discovery::IrisDiscovery` import and change the 3 existing test assertions to use `matches!(result, IrisDiscovery::Found(_))` pattern; this will FAIL TO COMPILE until T010 changes the function signature — that compile failure IS the valid RED state (WRITE FIRST)

### TDD Gate

- [x] T008 **GATE**: Run `cargo test -p iris-dev-core 2>&1 | grep "error\[E"` — confirm T006 and T007 produce type-mismatch compile errors (not runtime failures). The enum variants exist but `discover_iris()` signature has not changed yet. Do not proceed to T009 until both files fail to compile.

### Implementation for Phase 2

- [x] T009 Change `discover_via_docker_named()` signature from `-> Option<IrisConnection>` to `-> DiscoveryResult` in `crates/iris-dev-core/src/iris/discovery.rs` — update internal logic to return `DiscoveryResult::NotFound` where it previously returned `None`, and `DiscoveryResult::Connected(conn)` where it returned `Some(conn)`; leave `FoundUnhealthy` returning `DiscoveryResult::NotFound` for now (distinguish in later phases)
- [x] T010 Change `discover_iris()` signature from `-> Result<Option<IrisConnection>>` to `-> IrisDiscovery` in `crates/iris-dev-core/src/iris/discovery.rs` — update match arms: `DiscoveryResult::Connected(c)` → `IrisDiscovery::Found(c)`, `DiscoveryResult::NotFound` → continue cascade, `DiscoveryResult::FoundUnhealthy(_)` → `IrisDiscovery::NotFound` for now; end of cascade returns `IrisDiscovery::NotFound`
- [x] T011 Update `crates/iris-dev-bin/src/cmd/mcp.rs` — replace `discover_iris(explicit).await?` match with `IrisDiscovery` pattern-match per `contracts/discovery-api.md`; `Explained` branch → `None` silently; `NotFound` branch → keep existing warn
- [x] T012 Update `crates/iris-dev-bin/src/cmd/compile.rs` — replace `discover_iris(explicit).await?.context(...)` with explicit `IrisDiscovery` match per `contracts/discovery-api.md`; `Explained` → `std::process::exit(1)`; `NotFound` → `anyhow::bail!(...)`
- [x] T013 Update `crates/iris-dev-core/tests/discovery_tests.rs` — fix 3 existing tests to use `IrisDiscovery` pattern matching
- [x] T014 Verify `cargo test -p iris-dev-core` and `cargo build -p iris-dev` both pass — all existing tests green, binary compiles

**Checkpoint**: All existing tests pass with new return types. No behavior change yet.

---

## Phase 3: User Story 1 — Container not found (Priority: P1)

**Goal**: When `IRIS_CONTAINER` names a non-existent container, emit `"Container '{name}' not found in Docker"` and continue cascade.

**Independent Test**: `IRIS_CONTAINER=nonexistent iris-dev mcp` — stderr shows "not found in Docker", NOT "not reachable via Docker".

### Tests for US1 (write first — must FAIL before implementation)

- [x] T015 [P] [US1] Write unit test: `discover_via_docker_named("nonexistent")` against empty container list returns `DiscoveryResult::NotFound` in `crates/iris-dev-core/tests/unit/test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T016 [P] [US1] Write unit test: `discover_iris()` with `IRIS_CONTAINER=nonexistent` + empty Docker list → `IrisDiscovery::NotFound`, cascade continues (check that localhost scan is attempted by mocking probe_atelier) in `crates/iris-dev-core/tests/unit/test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T017 [US1] Write E2E test (no `#[ignore]`): start no containers; set `IRIS_CONTAINER=definitely-not-running`; run iris-dev mcp with tracing capture; assert stderr contains "not found in Docker" and does NOT contain "not reachable" in `crates/iris-dev-core/tests/docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T018 [US1] **GATE**: Confirm T015–T017 all FAIL

### Implementation for US1

- [x] T019 [US1] In `discover_via_docker_named()` in `discovery.rs`: when bollard `list_containers` succeeds but no container matches the name, return `DiscoveryResult::NotFound` (already done in T009 — verify it's correct)
- [x] T020 [US1] In `discover_iris()` in `discovery.rs`: when `discover_via_docker_named()` returns `DiscoveryResult::NotFound`, emit `tracing::warn!("Container '{}' not found in Docker — is it running? ('docker ps' to check)", container_name)` then continue to Step 4 (replace current generic warn at line 155-158)
- [x] T021 [US1] **GATE-GREEN**: Run `cargo test --test docker_discovery_e2e us1` — T017 must pass

**Phase gate**: T017 E2E passes. "not found in Docker" message confirmed; cascade continues.

---

## Phase 4: User Story 3 — Port not mapped (Priority: P1)

**Goal**: When container exists but port 52773 has no host mapping, emit port-not-mapped message and stop cascade.

**Independent Test**: `docker run -d --name repro-nomapped intersystemsdc/iris-community:latest --check-caps false` (no `-p 52773:...`); `IRIS_CONTAINER=repro-nomapped iris-dev mcp` — stderr shows port not mapped.

### Tests for US3 (write first — must FAIL before implementation)

- [x] T022 [P] [US3] Write unit test: `discover_via_docker_named("test")` with container present but `port_web=None` returns `DiscoveryResult::FoundUnhealthy(FailureMode::PortNotMapped)` in `crates/iris-dev-core/tests/unit/test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T023 [P] [US3] Write unit test: `discover_iris()` with `IRIS_CONTAINER` set and `FoundUnhealthy(PortNotMapped)` returns `IrisDiscovery::Explained` — cascade does NOT continue (no localhost probe attempted) in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T024 [US3] Write E2E test (no `#[ignore]`): start `iris-community:2026.1` WITHOUT `-p 52773:...`; run iris-dev; assert stderr contains "port 52773 is not mapped" and contains "iris_execute and iris_test still work" in `docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T025 [US3] **GATE**: Confirm T022–T024 all FAIL

### Implementation for US3

- [x] T026 [US3] In `discover_via_docker_named()` in `discovery.rs`: when container found but `port_web` is `None`, return `DiscoveryResult::FoundUnhealthy(FailureMode::PortNotMapped)` (replace current silent fallthrough at line 279)
- [x] T027 [US3] In `discover_iris()` in `discovery.rs`: when `FoundUnhealthy(PortNotMapped)` received, emit warn with port-not-mapped message + docker exec note (from `data-model.md` message templates), return `IrisDiscovery::Explained` — do NOT continue cascade
- [x] T028 [US3] **GATE-GREEN**: Run `cargo test --test docker_discovery_e2e us3` (requires running container) — T024 must pass

**Phase gate**: T024 E2E passes. Port-not-mapped message confirmed; cascade stops.

---

## Phase 5: User Story 2 — Web server not responding (Priority: P1)

**Goal**: When container + port found but Atelier probe fails (enterprise image, crashed web server), emit web-server-absent message and stop cascade.

**Independent Test**: Start `iris:2026.1` (enterprise) with port mapped; `IRIS_CONTAINER=repro-enterprise iris-dev mcp` — stderr shows "Atelier REST API is not responding" with enterprise hint.

### Tests for US2 (write first — must FAIL before implementation)

- [x] T029 [P] [US2] Write unit test: `discover_via_docker_named("test")` with container present, port mapped, probe returns connection refused → `DiscoveryResult::FoundUnhealthy(FailureMode::AtelierNotResponding { port: 52791 })` in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T030 [P] [US2] Write unit test: `discover_via_docker_named("test")` with container present, port mapped, probe returns HTTP 503 → `DiscoveryResult::FoundUnhealthy(FailureMode::AtelierHttpError { port: 52791, status: 503 })` in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T031 [P] [US2] Write unit test: `discover_iris()` with `FoundUnhealthy(AtelierNotResponding)` → `IrisDiscovery::Explained`, cascade stops in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T032 [US2] Write E2E test (`#[ignore]` — requires `IRIS_LICENSE_KEY_PATH`): start `iris:2026.1` with port mapped; run iris-dev; assert stderr contains "Atelier REST API is not responding" and contains enterprise hint text in `docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T033 [US2] **GATE**: Confirm T029–T032 all FAIL

### Implementation for US2

- [x] T034 [US2] Introduce `probe_atelier_for_container()` helper in `discovery.rs` — wraps `probe_atelier_with_client()`, returns `DiscoveryResult` directly (not `Option<IrisConnection>`); handles connection-refused/timeout → `FoundUnhealthy(AtelierNotResponding)`, HTTP error → `FoundUnhealthy(AtelierHttpError)`, 401 → `FoundUnhealthy(AtelierAuth401)` (the 401 warn is emitted here with container name included), success → `Connected`. **Preservation note**: T026 already changed `discover_via_docker_named()` to return `FoundUnhealthy(PortNotMapped)` for the no-port case — T035 replaces the probe call only, do not touch the port-mapping branch added by T026
- [x] T035 [US2] Replace `probe_atelier()` call inside `discover_via_docker_named()` with `probe_atelier_for_container()` — thread container name + port through
- [x] T036 [US2] In `discover_iris()`: handle `FoundUnhealthy(AtelierNotResponding { port })` and `FoundUnhealthy(AtelierHttpError { port, status })` — emit respective warn messages from `data-model.md` templates, return `IrisDiscovery::Explained`
- [x] T037 [US2] **GATE-GREEN (MANDATORY — cannot skip)**: Run `IRIS_LICENSE_KEY_PATH=~/license/iris.key cargo test --test docker_discovery_e2e -- --ignored us2` — T032 must pass. This is a required manual step before Phase 6 can begin; record the test output in the PR description.

**Phase gate**: T032 E2E passes (enterprise). Web-server-absent message confirmed; cascade stops.

---

## Phase 6: User Story 4 — 401 deduplication (Priority: P1)

**Goal**: When container + port found and probe returns 401, emit exactly ONE warn (with container name) — suppress the second generic "not found or not reachable" warn.

**Independent Test**: Start `iris-community:2026.1` WITHOUT `-e IRIS_PASSWORD`; `IRIS_CONTAINER=repro-community iris-dev mcp` — stderr contains exactly one WARN mentioning 401.

### Tests for US4 (write first — must FAIL before implementation)

- [x] T038 [P] [US4] Write unit test: `discover_via_docker_named("test")` with 401 probe response → `DiscoveryResult::FoundUnhealthy(FailureMode::AtelierAuth401 { port })` in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T039 [P] [US4] Write unit test: `discover_iris()` with `FoundUnhealthy(AtelierAuth401)` → `IrisDiscovery::Explained`, and verify only ONE warn is emitted (capture tracing output) in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)
- [x] T040 [US4] Write E2E test (no `#[ignore]`): start `iris-community:2026.1` without `IRIS_PASSWORD`; run iris-dev; assert stderr has exactly one warn line mentioning "401" AND does NOT contain "not found or not reachable" in `docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T041 [US4] **GATE**: Confirm T038–T040 all FAIL

### Implementation for US4

- [x] T042 [US4] In `probe_atelier_for_container()` (from T034): on 401, emit updated warn that includes container name and port: `"IRIS at localhost:{port} returned 401 — container '{name}' may need IRIS_PASSWORD. Restart with: docker run -e IRIS_PASSWORD=SYS ..."`, return `DiscoveryResult::FoundUnhealthy(AtelierAuth401 { port })`
- [x] T043 [US4] In `discover_iris()`: handle `FoundUnhealthy(AtelierAuth401)` → return `IrisDiscovery::Explained` WITHOUT emitting any additional warn (the warn was already emitted in `probe_atelier_for_container`); remove the old generic warn at line 155-158 entirely
- [x] T044 [US4] **GATE-GREEN**: Run `cargo test --test docker_discovery_e2e us4` — T040 must pass

**Phase gate**: T040 E2E passes. Exactly one 401 warn confirmed; generic second warn absent.

---

## Phase 7: User Story 5 — Regression harness (Priority: P1)

**Goal**: Full 4-image regression harness verifying all failure modes in one test run.

**Independent Test**: `cargo test --test docker_discovery_e2e` (community) + `IRIS_LICENSE_KEY_PATH=~/license/iris.key cargo test --test docker_discovery_e2e -- --ignored` (enterprise).

### Scaffolding for US5 (helpers needed before tests compile)

- [x] T045 [P] [US5] Implement test helper `run_iris_dev_mcp_capture_stderr(container_name: &str, extra_env: &[(&str, &str)]) -> String` in `docker_discovery_e2e.rs` — spawns iris-dev mcp subprocess, sends initialize+notifications/initialized, captures stderr for 3 seconds, kills process. See `interop_e2e_tests.rs::mcp_exchange` for subprocess spawn pattern.
- [x] T046 [P] [US5] Implement `start_fresh_container(image: &str, name: &str, port_map: Option<(u16, u16)>, license_key: Option<&str>) -> String` helper in `docker_discovery_e2e.rs` — wraps `docker run`, returns container name, registered for cleanup via `docker rm -f` on drop

### Tests for US5 (write after helpers — must FAIL before T049/T050)

- [x] T047 [P] [US5] Write community regression test: `test_all_community_images` — spins up `iris-community:2026.1` and `irishealth-community:2026.1` fresh using T046 helper; runs iris-dev using T045 helper; asserts correct failure mode message for each in `docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)
- [x] T048 [P] [US5] Write enterprise regression test (`#[ignore]`): `test_all_enterprise_images` — spins up `iris:2026.1` and `irishealth:2026.1` with key using T046 helper; asserts "Atelier REST API not responding" + enterprise hint for each in `docker_discovery_e2e.rs` (WRITE FIRST, must FAIL)

### Implementation for US5

- [x] T049 [US5] **GATE-GREEN (community)**: Run `cargo test --test docker_discovery_e2e` — T047 must pass without license key
- [x] T050 [US5] **GATE-GREEN (enterprise — MANDATORY — cannot skip)**: Run `IRIS_LICENSE_KEY_PATH=~/license/iris.key cargo test --test docker_discovery_e2e -- --ignored` — T046 must pass. Record output in PR description before merging.

**Phase gate**: Both T049 and T050 pass. Full 4-image harness green.

---

## Phase 8: FR-007 — Localhost scan credential fix (Priority: P1)

**Goal**: Localhost port scan uses `IRIS_USERNAME`/`IRIS_PASSWORD` env vars instead of hardcoded `_SYSTEM`/`SYS`.

### Tests

- [x] T051 [P] Write unit test: set `IRIS_USERNAME=myuser` and `IRIS_PASSWORD=mypass`; mock localhost:52773 to return 200 only for those credentials; assert `discover_iris()` returns `IrisDiscovery::Found` in `test_discovery_unit.rs` (WRITE FIRST, must FAIL)

### Implementation

- [x] T052 In `discover_iris()` in `discovery.rs` — replace hardcoded `"_SYSTEM"`, `"SYS"`, `"USER"` in the localhost scan loop (line ~176) with env var reads per `contracts/discovery-api.md`
- [x] T053 **GATE-GREEN**: Run `cargo test --test test_discovery_unit` — T051 passes

**Phase gate**: T051 passes. Localhost scan uses env var credentials.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [x] T054 [P] Run full test suite: `cargo test -p iris-dev-core` — all unit tests pass, zero regressions in `test_toolset`, `test_compile_params`, `discovery_tests`, `interop_unit_tests`
- [x] T055 [P] Run `cargo build --release -p iris-dev` — binary compiles cleanly, no new warnings
- [x] T056 [P] Verify `cargo test --test test_discovery_unit` — all new unit tests pass
- [x] T057 Clean up the four repro containers from manual testing: `docker rm -f repro-community-2026 repro-enterprise-2026 repro-irishealth-community repro-irishealth-enterprise repro-enterprise-fixed 2>/dev/null; echo done`
- [x] T058 Update issue #28 with a comment linking to the branch and summarizing what was found (enterprise image root cause, CPF merge doesn't work, correct fix is external web gateway)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — blocks all user story phases
- **Phase 3 (US1 — not found)**: Depends on Phase 2
- **Phase 4 (US3 — port not mapped)**: Depends on Phase 2; can run concurrently with Phase 3
- **Phase 5 (US2 — web server down)**: Depends on Phase 4 completing first — T034 modifies the same `discover_via_docker_named()` function as T026; T026 must be done before T034 or the `PortNotMapped` return path will be overwritten
- **Phase 6 (US4 — 401 dedup)**: Depends on Phase 5 (shares `probe_atelier_for_container`)
- **Phase 7 (US5 — regression harness)**: Depends on Phases 3–6 all complete
- **Phase 8 (FR-007 — localhost credentials)**: Independent of Phases 3–7; can run after Phase 2
- **Phase 9 (Polish)**: Depends on all phases complete

### User Story Dependencies

- **US1** (not found): Only needs `DiscoveryResult::NotFound` path — can start after Phase 2
- **US3** (port not mapped): Only needs `FoundUnhealthy(PortNotMapped)` — can start after Phase 2 in parallel with US1
- **US2** (web server down): Needs `probe_atelier_for_container()` from T034 — depends on T034 existing
- **US4** (401 dedup): Shares `probe_atelier_for_container()` — depends on T034; most efficient to do after US2

### Critical Path

```
T001-T005 (setup) → T006-T014 (foundational) → T015-T021 (US1) ┐
                                               → T022-T028 (US3) ┤→ T034 (probe helper) → T035-T037 (US2) → T038-T044 (US4) → T045-T050 (US5)
                                               → T051-T053 (FR-007, parallel)
```

---

## Parallel Opportunities

### Phase 2 — tests written in parallel

```
T006: unit test — IrisDiscovery::NotFound
T007: update existing discovery_tests.rs for new types
[both in parallel, different concerns]
```

### Phases 3 + 4 — run concurrently after Phase 2

```
Developer A: Phase 3 (US1 — not found message)
Developer B: Phase 4 (US3 — port not mapped message)
```

### Phase 9 — all polish tasks [P] in parallel

```
T054: full test suite
T055: release build
T056: unit test suite
T057: container cleanup
T058: issue update
```

---

## Implementation Strategy

### MVP: Phases 1–3 (container not found message fixed)

1. Add enum stubs (Phase 1)
2. Wire new return types without behavior change (Phase 2)
3. Fix "not found" message text (Phase 3)
4. **STOP AND VALIDATE**: `IRIS_CONTAINER=nonexistent iris-dev mcp` shows "not found in Docker" not "not reachable via Docker"

This MVP is independently useful — it fixes the most confusing case (user typos the container name) without touching the Atelier probe path.

### Full Feature

5. Phase 4: Port-not-mapped message + cascade stop
6. Phase 5: Web-server-absent message (enterprise fix) — the core #28 issue
7. Phase 6: 401 dedup — one clean message instead of two
8. Phase 7: Full 4-image regression harness
9. Phase 8: Localhost scan credentials
10. Phase 9: Polish + issue update
