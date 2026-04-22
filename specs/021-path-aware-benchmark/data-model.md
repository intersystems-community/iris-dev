# Data Model — Spec 021 Path-Aware Benchmark

## Core Entities

### BenchmarkTask
The atomic unit of evaluation. Defined in YAML, executed by the runner.

```yaml
id: GEN-01                        # category-sequence
category: GEN                     # GEN | MOD | DBG | SCM | LEG
description: "Write a class..."   # prompt sent to the agent
path: both                        # A | B | both
expected_behavior: "..."          # what correct output looks like (for LLM judge)
fixtures:                         # IRIS state to set up before task runs
  - type: cls                     # cls | global | routine | namespace
    content: "..."
cleanup:                          # what to kill after (usually just namespace wipe)
  - type: namespace
    name: BENCHMARK
tags: [basic, no-context]         # for filtering runs
```

### BenchmarkRun
One full execution of the benchmark suite (or a subset).

```json
{
  "run_id": "2026-04-22T14:00:00Z",
  "iris_dev_version": "0.2.0",
  "harness": "claude-code",
  "path": "A",
  "task_filter": null,
  "tasks": [...],
  "summary": {...}
}
```

### TaskResult
One task execution within a run.

```json
{
  "task_id": "GEN-01",
  "category": "GEN",
  "path": "A",
  "harness": "claude-code",
  "score": 3,
  "reasoning": "...",
  "tool_calls": ["iris_compile", "iris_execute"],
  "tool_call_count": 2,
  "scm_elicitation_triggered": false,
  "duration_ms": 4200,
  "raw_transcript": "..."
}
```

### RunSummary
Aggregated scores across all tasks in a run.

```json
{
  "mean_score_overall": 2.4,
  "mean_score_path_a": 2.6,
  "mean_score_path_b": 1.4,
  "by_category": {
    "GEN": {"path_a": 2.8, "path_b": 1.6},
    "MOD": {"path_a": 2.4, "path_b": 1.2},
    "DBG": {"path_a": 2.6, "path_b": 1.8},
    "SCM": {"path_a": 2.2, "path_b": 2.0},
    "LEG": {"path_a": 1.8, "path_b": 1.4}
  },
  "task_count": 25,
  "pass_rate": 0.76
}
```


## File Layout (iris-dev repo)

```
benchmark/
  021/
    tasks/
      GEN-01.yaml
      GEN-02.yaml
      ...
      LEG-01.yaml    # Steve P's .mac + globals scenario
    runner/
      __main__.py    # entry point: python -m benchmark.021.runner
      claude_code.py # MCP stdio driver
      copilot.py     # Playwright driver (Copilot harness)
      judge.py       # LLM-as-judge scoring
      namespace.py   # BENCHMARK namespace setup/teardown
      report.py      # JSON + HTML report generator
    results/
      .gitkeep       # results committed per run
    report_template.html
```


## Task YAML Validation Rules

- `id` must match pattern `[A-Z]+-[0-9]+`
- `category` must be one of: GEN, MOD, DBG, SCM, LEG
- `path` must be one of: A, B, both
- `description` required, non-empty
- `expected_behavior` required, non-empty
- `fixtures` optional, each must have `type` field
- Tasks with `path: A` are skipped when runner is invoked with `--path B` and vice versa


## Skill Tag Extension

Existing skill YAML files get a new optional field:
```yaml
path: local   # local | isfs | both
```
Default if absent: `both`. Benchmark runner uses this to filter which skills are loaded into the agent context per path.
