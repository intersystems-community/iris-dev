# Tasks — Spec 021 Path-Aware Benchmark

## Phase 1: Setup

- [ ] T001 Create benchmark/021/ directory structure per plan.md (tasks/, runner/, results/, report_template.html)
- [ ] T002 Create benchmark/021/runner/requirements.txt with: anthropic, pyyaml, jinja2
- [ ] T003 Create benchmark/021/runner/__main__.py with CLI arg parsing (--path, --categories, --task, --harness, --report-only)
- [ ] T004 [P] Create benchmark/021/runner/namespace.py — BENCHMARK namespace create/wipe via iris_execute
- [ ] T005 [P] Create benchmark/021/runner/judge.py — LLM-as-judge scaffold using claude-haiku-4-5, returns {score, reasoning}

## Phase 2: Foundational

- [ ] T006 Write unit test: test_namespace.py — verify BENCHMARK namespace creates and wipes cleanly
- [ ] T007 Write unit test: test_judge.py — verify judge returns valid {score: 0-3, reasoning: str} for a mock transcript
- [ ] T008 Write unit test: test_task_loader.py — verify YAML loads, validates, filters by --path and --category
- [ ] T009 [P] Implement task loader in benchmark/021/runner/task_loader.py — load/validate all tasks/*.yaml, filter flags
- [ ] T010 [P] Implement fixture applier in benchmark/021/runner/fixtures.py — apply cls/global/routine fixtures to BENCHMARK namespace
- [ ] T011 E2E gate: run test_namespace.py + test_task_loader.py + test_judge.py — all must pass before Phase 3

## Phase 3: US1 — Claude Code harness, Path A, GEN category

Story goal: Full end-to-end benchmark run for the blessed path. Five GEN tasks, scored, results written.

- [ ] T012 [US1] Write unit test: test_claude_code_driver.py — mock MCP stdio, verify tool call loop collects transcript correctly
- [ ] T013 [US1] Write E2E test: e2e_gen_path_a.py — run GEN-01 end-to-end against live IRIS, assert score >= 0
- [ ] T014 [P] [US1] Create benchmark/021/tasks/GEN-01.yaml — basic class Bench.Greeter, ClassMethod Hello() returns 'Hello World'
- [ ] T015 [P] [US1] Create benchmark/021/tasks/GEN-02.yaml — class that calls an existing class (tests iris_generate context lift)
- [ ] T016 [P] [US1] Create benchmark/021/tasks/GEN-03.yaml — persistent class with properties and %Save
- [ ] T017 [P] [US1] Create benchmark/021/tasks/GEN-04.yaml — class with embedded SQL SELECT query
- [ ] T018 [P] [US1] Create benchmark/021/tasks/GEN-05.yaml — class that reads/writes a global
- [ ] T019 [US1] Implement benchmark/021/runner/claude_code.py — MCP stdio driver: spawn iris-dev mcp, path system prompt injection, tool call loop, transcript capture
- [ ] T020 [US1] Implement benchmark/021/runner/result_writer.py — write scores.json incrementally after each task, write final report.html
- [ ] T021 [US1] Create benchmark/021/report_template.html — self-contained Jinja2 template, bar chart per category, score table, path comparison
- [ ] T022 [US1] E2E gate (Phase 3): run all 5 GEN tasks, Path A, Claude Code harness — all complete, scores.json + report.html written

## Phase 4: US2 — Path B + remaining categories (MOD, DBG, SCM, LEG)

Story goal: Full benchmark coverage both paths. All 5 categories. Publishable comparison data.

- [ ] T023 [US2] Write unit test: test_path_b_prompt.py — verify Path B system prompt contains iris_doc and no local-file instructions
- [ ] T024 [US2] Write E2E test: e2e_gen_path_b.py — run GEN-01 on Path B, verify iris_doc put called in transcript
- [ ] T025 [P] [US2] Create benchmark/021/tasks/MOD-01.yaml through MOD-05.yaml — modification tasks with fixture classes pre-loaded
- [ ] T026 [P] [US2] Create benchmark/021/tasks/DBG-01.yaml through DBG-05.yaml — debug tasks with buggy fixture class + error log
- [ ] T027 [P] [US2] Create benchmark/021/tasks/SCM-01.yaml through SCM-03.yaml — SCM elicitation tasks (skip gracefully if no SCM package)
- [ ] T028 [P] [US2] Create benchmark/021/tasks/LEG-01.yaml — Steve P scenario: .mac routine with globals-only data model, no classes
- [ ] T029 [P] [US2] Create benchmark/021/tasks/LEG-02.yaml through LEG-05.yaml — additional legacy patterns
- [ ] T030 [US2] Add Path B system prompt to benchmark/021/runner/claude_code.py — "ISFS mode: use iris_doc, do NOT edit local files"
- [ ] T031 [US2] Add SCM skip logic to runner/__main__.py — detect SCM unavailable via iris_source_control status, skip SCM tasks gracefully
- [ ] T032 [US2] E2E gate (Phase 4): run full benchmark both paths, Claude Code harness — scores.json with path A vs B comparison, report.html generated
- [ ] T033 [US2] Update README.md "Path Comparison" section with actual results from E2E gate run

## Phase 5: US3 — Copilot harness (vscode-test-electron)

Story goal: Same benchmark tasks runnable via Copilot agent mode. Three-way comparison: CC Path A, CC Path B, Copilot Path A, Copilot Path B.

- [ ] T034 [US3] Write unit test: test_copilot_driver.py — mock vscode-test subprocess, verify result JSON parsed correctly
- [ ] T035 [US3] Create benchmark/021/runner/copilot-driver/ — TypeScript project scaffold: package.json, tsconfig.json, src/index.ts
- [ ] T036 [US3] Implement copilot-driver/src/index.ts — VS Code Extension Test Host, submit task via vscode.commands.executeCommand('github.copilot.sendToChat'), poll for completion, write result JSON to stdout
- [ ] T037 [US3] Implement benchmark/021/runner/copilot.py — Python subprocess wrapper for copilot-driver, same interface as claude_code.py
- [ ] T038 [US3] E2E gate (Phase 5): run GEN-01 through GEN-05, Path A, Copilot harness — scores captured, three-way comparison report generated

## Phase 6: Polish

- [ ] T039 Add --dry-run flag to runner/__main__.py — loads tasks and prints plan without executing
- [ ] T040 Add benchmark/021/README.md — how to run, prerequisites, what each category tests, how to add new tasks
- [ ] T041 Add task count validation to CI — ci.yml runs `python -m benchmark.021.runner --dry-run` to verify task YAML parses clean on every push
- [ ] T042 Commit first real results to benchmark/021/results/ with both paths documented

## Dependencies

```
Phase 1 (Setup) → Phase 2 (Foundation) → Phase 3 (US1) → Phase 4 (US2) → Phase 5 (US3) → Phase 6 (Polish)
```

Within Phase 3, T019 (claude_code.py) depends on T009 (task_loader). All task YAML files (T014-T018) are parallel. T022 (E2E gate) depends on all of T012-T021.

Within Phase 4, task YAML files (T025-T029) are all parallel and can be written alongside Phase 3.

## Implementation Strategy

MVP = Phase 3 complete (T001-T022): One harness, one path, one category, end-to-end with scored results. This is the minimum needed to show at READY with real data.

Full publish = Phase 4 complete (T033): Both paths, all categories, comparison data in README.

Copilot harness (Phase 5) is post-READY work unless vscode-test-electron proves straightforward.
