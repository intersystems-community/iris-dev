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

1. Claude Code (primary — this is where iris-dev is already integrated)
2. GitHub Copilot agent mode (VS Code extension wires iris-dev in)
3. Cursor (via MCP config — future, stretch goal)


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
Expected to stress both paths differently.


## Scoring

Each task scored 0–3:
- 0: Failed or wrong output
- 1: Partial — compiled but incorrect behavior
- 2: Correct but required >2 tool calls that shouldn't have been needed (agent confusion)
- 3: Correct, efficient, no unnecessary tool calls

Report: mean score per category × path × harness. Also report: tool call count, error rate, SCM elicitation triggered (Y/N).


## Success Criteria

- Benchmark runs end-to-end unattended for both paths
- Results published in benchmark/results/021/ as JSON + markdown summary
- README updated with "Path Comparison" section linking to results
- At least 5 tasks per category, both paths, Claude Code harness (minimum viable publish)
- Copilot harness results included before public announcement


## Explicit Non-Goals

- Not picking a winner by fiat — the data picks the winner
- Not deprecating Path B until data shows clear gap AND migration path exists
- Not testing every possible harness before publishing — ship Claude Code results first


## Connection to Skills

Each skill in the skills registry will be tagged with:
- `path: local` — only applies to Path A
- `path: isfs` — only applies to Path B
- `path: both` — path-agnostic

The benchmark results will inform which skills need path-specific variants vs. which are universal.


## Reference

- Existing benchmark: eval/agentic_eval/ in objectscript-coder repo
- iris-dev benchmark tool: mcp__objectscript-plaza__benchmark_agentic_eval
- Nathan's finding: TC-519522 (ISC JIRA)
- Council discussion: 2026-04-22
