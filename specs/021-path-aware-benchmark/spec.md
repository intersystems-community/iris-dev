# Spec 021 — Path-Aware Agentic Benchmark

## Context

Emerging from a council discussion (Tom, Nathan Keast, Tim Leavitt, Brett Saviano, Steve Pisani) on what the right AI coding path is for IRIS developers. Nathan's empirical finding: ISFS+Atelier MCP produces poor outcomes due to three competing edit paths; local-files+Atelier MCP produces good outcomes with a single edit path. The group agreed: don't argue, measure. Publish the results.

## Goal

Extend the existing agentic eval harness to benchmark two explicit development paths across multiple AI harnesses and task types. Produce scored, repeatable, publishable results that answer: "which path works better, and by how much?"


## The Two Paths

### Path A — Local Files + Atelier (blessed path)
- Agent edits .cls files on local filesystem
- iris-dev provides: compile, execute, test, search, introspect, SCM hooks
- No ISFS workspace open
- Write path: local file → iris_compile → IRIS

### Path B — ISFS Only (legacy/remote-only path)
- No local files — code lives only in IRIS
- iris-dev provides: iris_doc (read/write), compile, execute, search, introspect, SCM hooks
- ISFS workspace may or may not be open in VS Code
- Write path: iris_doc put → IRIS

Path B is labeled "legacy support" in docs. The benchmark exists to quantify the gap and make the migration case with data.


## Harnesses Under Test

1. Claude Code — driven via MCP stdio (primary, unattended)
2. GitHub Copilot agent mode — driven via Playwright automating VS Code UI
3. Cursor (via MCP config — future, stretch goal)

Harness automation: dual driver architecture. MCP stdio for Claude Code (no browser needed). Playwright for Copilot/VS Code harnesses. Both share the same task definitions, scoring interface, and result schema.


## Task Categories

### GEN — Code Generation
Tasks that require writing new ObjectScript classes from a description.
Variants: pure generation (no context needed) vs. generation that requires finding existing classes to call.

### MOD — Modification
Tasks that require reading existing code, understanding it, and making a targeted change.
Path A advantage expected: agent can grep local files. Path B must use iris_search.

### DBG — Debug
Tasks that require diagnosing a runtime error from logs and fixing the root cause.

### SCM — Source Control
Tasks involving checkout, checkin, SCM-aware writes. Only meaningful on Path B or hybrid.

### LEG — Legacy Patterns
Tasks involving .mac routines, globals, no-class code. Steve P's customer scenario.
First task: GEN-LEG-01 — .mac routine with globals-only data model (no classes).
Expected to stress both paths differently.


## Scoring

Each task scored 0–3 by LLM-as-judge using a fixed rubric prompt:
- 0: Failed or wrong output
- 1: Partial — compiled but incorrect behavior
- 2: Correct but required >2 tool calls that shouldn't have been needed (agent confusion)
- 3: Correct, efficient, no unnecessary tool calls

Report: mean score per category × path × harness. Also report: tool call count, error rate, SCM elicitation triggered (Y/N).


## Task Isolation

Dedicated `BENCHMARK` namespace in the existing iris-dev-iris container. Wiped between tasks via IRIS namespace kill. No impact on USER or any other namespace. Benchmark runner creates the namespace on first run if absent.


## Results Location

Benchmark lives in `iris-dev` repo under `benchmark/021/`. Results published as:
- `results/<run-id>/scores.json` — machine-readable per-task scores
- `results/<run-id>/report.html` — generated HTML report (visual-explainer style), shareable at READY and linkable from README

README gets a "Path Comparison" section linking to latest results.


## Success Criteria

- Benchmark runs end-to-end unattended for both paths, Claude Code harness
- Results published as JSON + HTML in benchmark/021/results/
- README updated with "Path Comparison" section
- At least 5 tasks per category, both paths, Claude Code harness (minimum viable publish)
- Copilot harness (Playwright) results included before public announcement
- GEN-LEG-01 (.mac + globals) task implemented and scored on both paths


## Explicit Non-Goals

- Not picking a winner by fiat — the data picks the winner
- Not deprecating Path B until data shows clear gap AND migration path exists
- Not testing every possible harness before publishing — ship Claude Code results first
- Not running on CI automatically (local IRIS required) — manual trigger only


## Connection to Skills

Each skill in the skills registry will be tagged with:
- `path: local` — only applies to Path A
- `path: isfs` — only applies to Path B
- `path: both` — path-agnostic

The benchmark results will inform which skills need path-specific variants vs. which are universal.


## Clarifications

### Session 2026-04-22
- Q: Who or what determines the 0-3 score? → A: LLM-as-judge with a fixed rubric prompt
- Q: How do we handle harness automation given Copilot requires VS Code UI? → A: Dual driver — MCP stdio for Claude Code, Playwright for Copilot/VS Code harnesses, shared task runner and scorer
- Q: Where does the benchmark live? → A: iris-dev repo, benchmark/021/ directory, first-class alongside the Rust code
- Q: What format should the published report take? → A: JSON + generated HTML report
- Q: What is the task isolation/cleanup mechanism? → A: Dedicated BENCHMARK namespace, wiped between tasks via IRIS namespace kill


## Reference

- Existing benchmark: eval/agentic_eval/ in objectscript-coder repo (reference only — this spec supersedes it for path comparison work)
- Nathan's finding: TC-519522 (ISC JIRA)
- Council discussion: 2026-04-22
