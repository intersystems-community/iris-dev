# Feature Specification: GEPA + RLM Skill Optimization Loop

**Feature Branch**: `030-gepa-skill-optimizer` (not yet created)
**Created**: 2026-05-02
**Status**: Sketch — not ready to implement

## Overview

Close the loop between the benchmark harness (029-tool-ablation-study), the skill registry
(`skill_propose`/`skill_optimize`), and GEPA's `optimize_anything` API. The optimizer:

1. Runs the benchmark task suite against the current skill set (or a subset)
2. Captures execution traces (tool calls, scores, reasoning)
3. Uses an RLM to analyze traces across many runs without flooding context
4. Feeds structured findings to GEPA `optimize_anything` to evolve skill text + tool descriptions
5. Scores the candidate improvements on a held-out task set
6. Commits winning candidates back to the skill registry

The Lespérance "RLM ⊕ GEPA" result (April 2026): a predict-RLM can improve another
predict-RLM using GEPA — the optimizer itself can be recursively improved. The HALO paper
(April 2026) demonstrated +26.9% on AppWorld (Sonnet 4.6) using this exact pattern on
execution traces.

## Key References from Bookmarks

- **Lespérance (April 25)**: "RLM ⊕ GEPA: You can use RLMs to improve RLMs with GEPA"
  — SpreadsheetBench: RLM_gpt-5.5-medium reaches 0.8925 hard
- **HALO (April 29)**: Hierarchical Agent Loop Optimizer — RLM-based agent optimization
  via execution trace analysis, AppWorld Sonnet 4.6: baseline → improved
- **intertwine/dspy-agent-skills (April 21)**: "DSPy + GEPA + RLM in Claude and Codex in
  minutes" — 130+ stars, working reference implementation
- **Shawn Tenam (April 20)**: GEPA bumped Haiku 4.5 from 65% to 85% by auto-optimizing
  CLAUDE.md-style instructions
- **GEPA for Skills / gskill (Feb 19)**: Shangyin Tan — automated pipeline to learn agent
  skills, near-perfect repo task resolution, 47% faster

## Sketch: Components

### In objectscript-coder (Python benchmark harness)

```
benchmark/030/
├── runner.py          # run benchmark, capture traces + scores
├── trace_analyzer.py  # RLM-style iteration over traces (no context flooding)
├── gepa_optimizer.py  # call GEPA optimize_anything with trace-derived findings
├── skill_updater.py   # write winning candidates back to iris-dev skill registry
└── eval.py           # hold-out set scoring for candidate validation
```

### In iris-dev (Rust)

- `skill_optimize` already exists as a stub — needs a real implementation that accepts
  GEPA-generated candidate skill text and validates it against the benchmark

## The Loop

```
1. Run benchmark (GEN/MOD/DBG tasks) → traces.jsonl + scores.json
2. RLM trace analyzer:
   - Pages through traces in batches (no context flood)
   - Extracts: which tool calls failed, which skills were invoked, score deltas
   - Builds structured "failure patterns" per skill
3. GEPA optimize_anything:
   - Input: current skill text + failure patterns
   - Metric: benchmark score on validation set
   - Output: evolved skill text candidates
4. Score candidates on held-out set
5. Accept winners (score improvement ≥ threshold)
6. Commit to skill registry via skill_optimize / skill_share
7. (Optional) Run GEPA on the optimizer itself — Lespérance pattern
```

## Open Questions (not resolved)

- Which GEPA endpoint? `optimize_anything` API is at `gepa-ai.github.io` — need API key.
  Alternatively use the `dspy-agent-skills` repo's open-source implementation.
- What's the "metric function" for GEPA? Benchmark score is the obvious choice but it's
  expensive (each evaluation = full benchmark run). May need a cheaper proxy metric.
- How many skills to optimize per run? All at once risks interference; one at a time is
  slow. The Databricks routing case (75% gains) optimized one component at a time.
- Should this live in objectscript-coder (Python, near the benchmark) or iris-dev (Rust,
  near the skill registry)? Likely Python harness calls iris-dev tools for skill updates.
- How does the RLM fit? Options: (a) RLM analyzes traces server-side in iris-dev, (b)
  Python benchmark harness implements the RLM loop directly, (c) Use dspy.RLM from the
  intertwine repo as a library.

## Prerequisites

- Feature 029 (tool ablation study benchmark harness) — SHIPPED ✅
- GEPA API access or dspy-agent-skills local implementation
- Skill registry with `skill_optimize` wired (currently stub in iris-dev)
