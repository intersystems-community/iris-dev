# Contract — Benchmark Runner CLI

## Invocation

```bash
# Run all tasks, both paths, Claude Code harness
python -m benchmark.021.runner

# Run specific path
python -m benchmark.021.runner --path A
python -m benchmark.021.runner --path B

# Run specific categories
python -m benchmark.021.runner --categories GEN,MOD

# Run specific harness
python -m benchmark.021.runner --harness claude-code
python -m benchmark.021.runner --harness copilot

# Run single task (development/debug)
python -m benchmark.021.runner --task GEN-01

# Generate report from existing results
python -m benchmark.021.runner --report-only results/2026-04-22T14:00:00Z/scores.json
```

## Environment Variables Required

```bash
IRIS_HOST=localhost
IRIS_WEB_PORT=52780
IRIS_USERNAME=_SYSTEM
IRIS_PASSWORD=SYS
ANTHROPIC_API_KEY=...        # for Claude Code harness + LLM judge
```

## Exit Codes

- 0: All tasks scored ≥ 2 (passing)
- 1: One or more tasks scored 0 or 1 (failures)
- 2: Runner error (IRIS unreachable, missing env var, etc.)

## Output

On completion, prints summary table to stdout and writes:
- `benchmark/021/results/<run-id>/scores.json`
- `benchmark/021/results/<run-id>/report.html`
