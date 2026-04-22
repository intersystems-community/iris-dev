# Implementation Plan — Spec 021 Path-Aware Benchmark

## Architecture

```
benchmark/021/
├── tasks/              # YAML task definitions (source of truth)
├── runner/             # Python benchmark runner
│   ├── __main__.py     # CLI entry point
│   ├── claude_code.py  # MCP stdio driver (Anthropic API + tool loop)
│   ├── copilot.py      # Playwright driver (VS Code UI automation)
│   ├── judge.py        # LLM-as-judge scoring via Claude API
│   ├── namespace.py    # BENCHMARK namespace setup/teardown
│   └── report.py       # JSON + HTML report generator
├── results/            # Run results committed here
└── report_template.html
```

## Phase 1 — Foundation (Claude Code harness, Path A only)

Goal: end-to-end working benchmark for the blessed path. Green before expanding.

### T001 — Create benchmark directory structure and runner skeleton
- `benchmark/021/runner/__main__.py` with CLI arg parsing
- `benchmark/021/runner/namespace.py` — create/wipe BENCHMARK namespace via iris_execute
- Validates IRIS_HOST, IRIS_WEB_PORT, ANTHROPIC_API_KEY env vars on startup

### T002 — Implement task loader
- Load and validate all `tasks/*.yaml` files against schema
- Filter by --path, --categories, --task flags
- Fail fast on schema violations

### T003 — Implement Claude Code driver (MCP stdio)
- Spawns `iris-dev mcp` subprocess with IRIS env vars + path-appropriate system prompt
- System prompt for Path A declares: "You are working in local-files mode. Edit .cls files on the local filesystem. Use iris_compile to compile. Do NOT use iris_doc put."
- System prompt for Path B declares: "You are working in ISFS mode. Use iris_doc to read and write documents. Do NOT edit local files."
- Sends MCP initialize handshake, then submits task prompt
- Collects all tool_use / tool_result turns until final text response
- Returns transcript: list of {role, tool_name, args, result}

### T004 — Implement LLM judge
- Takes task + transcript, calls Claude Haiku via Anthropic API
- Returns {score: 0-3, reasoning: str}
- Prompt includes task.expected_behavior and full transcript
- Retries once on API error

### T005 — Write first 5 GEN tasks (Path A)
- GEN-01: basic class generation, no context
- GEN-02: class with a method that calls an existing class (tests iris_generate context)
- GEN-03: persistent class with properties and %Save
- GEN-04: class with embedded SQL query
- GEN-05: class that reads/writes a global

### T006 — Wire fixture setup and namespace teardown
- Before each task: apply fixtures to BENCHMARK namespace
- After each task: kill ^* in BENCHMARK namespace via iris_execute
- Handle fixture failure gracefully (skip task, log error)

### T007 — Implement result writer and basic report
- Write scores.json after each task (incremental, not just at end)
- Generate markdown summary table to stdout
- Generate report.html from template (bar chart per category, score table)

### T008 — Phase 1 E2E gate: run all 5 GEN tasks, Path A, Claude Code
- All 5 tasks must complete (score >= 0, no runner crash)
- Mean score reported and stored
- Phase 1 complete when this passes

## Phase 2 — Path B + Remaining Categories

### T009 — Path B system prompt and iris_doc write path
- Add Path B system prompt to claude_code.py driver
- Verify iris_doc put works end-to-end in BENCHMARK namespace
- Run GEN-01 through GEN-05 on Path B and compare scores

### T010 — Write MOD tasks (5 tasks, both paths)
- MOD tasks require fixture classes to be pre-loaded
- Test agent's ability to find and modify existing code

### T011 — Write DBG tasks (5 tasks, both paths)
- Pre-load a class with a deliberate bug
- Inject error log fixture
- Test agent's ability to diagnose and fix

### T012 — Write SCM tasks (3 tasks, Path B primary)
- Test iris_source_control elicitation flow
- Requires SCM package active — skip gracefully if not present

### T013 — Write LEG tasks (5 tasks, both paths)
- LEG-01: Steve P .mac + globals scenario (see task-schema.md)
- LEG-02 through LEG-05: additional legacy patterns

### T014 — Phase 2 E2E gate: full benchmark, both paths, Claude Code
- All 23 tasks, both paths
- Path A vs Path B comparison published to results/
- README "Path Comparison" section written

## Phase 3 — Copilot Harness (Playwright)

### T015 — Prototype vscode-test-electron Copilot harness
- Copilot Chat pane uses WebView/Canvas — no stable DOM selectors, Playwright not viable
- Use @vscode/test-electron + vscode.commands.executeCommand('github.copilot.sendToChat')
- Prototype: launch VS Code Extension Test Host, submit one task prompt, capture response
- Python runner orchestrates via subprocess; TypeScript test script handles VS Code internals

### T016 — Implement Copilot driver (TypeScript extension test)
- benchmark/021/runner/copilot-driver/ — TypeScript project
- Submits task.description to Copilot Chat via vscode.commands
- Polls for response completion
- Captures iris-dev MCP tool call transcript from server stdio
- Writes result JSON to stdout for Python runner to collect

### T017 — Run full benchmark via Copilot harness
- Same 23 tasks, both paths
- Three-way comparison: Claude Code Path A, Claude Code Path B, Copilot Path A, Copilot Path B

### T018 — Phase 3 E2E gate + publish
- Full results JSON + HTML generated
- README updated with actual numbers
- Results committed to repo

## Key Decisions

- Runner language: Python (consistent with existing eval harness, Playwright Python bindings)
- Judge model: claude-haiku-4-5 (fast, cheap, ObjectScript-aware)
- Harness isolation: BENCHMARK namespace wipe between tasks (not container restart)
- Results committed to repo (not external storage) — makes history visible in git log
- Playwright approach for Copilot: TBD pending research agent result (see research.md R3)

## Dependencies

- Python 3.11+
- anthropic SDK (already in objectscript-mcp)
- playwright (new dep for Phase 3 only)
- pyyaml
- jinja2 (for HTML report template)
- iris-dev binary on PATH
