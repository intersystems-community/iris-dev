# Benchmarking ObjectScript AI Skills

Run the repair benchmark yourself, measure your skills, and submit results to the leaderboard.

**Time required**: ~10 minutes for setup, ~5 minutes per skill run.

---

## Prerequisites

1. **Docker** — IRIS runs in a container
2. **Python 3.11+** and `pip` — the harness is Python
3. **AWS credentials** (for Bedrock LLM) — or an OpenAI API key as an alternative
4. **The benchmark repo**

```bash
git clone https://gitlab.iscinternal.com/devx/iris-dev.git
cd iris-dev
```

---

## Quick Start — Run One Skill in 10 Minutes

```bash
# 1. Start the IRIS benchmark container
docker run -d --name iris-bench \
  -p 1972:1972 -p 52773:52773 \
  intersystemsdc/iris-community:latest
# Wait ~30 seconds for IRIS to start

# 2. Install the benchmark harness
pip install -e objectscript_mcp/

# 3. Run the benchmark with the top-ranked skill
BENCH_MODEL=us.anthropic.claude-sonnet-4-6 \
IRIS_CONTAINER=iris-bench \
python3 bench/run_benchmark.py \
  --skill light-skills/skills/objectscript-review/SKILL.md \
  --baseline \
  --output results.json

# 4. See your results
cat results.json | python3 -c "
import json,sys
d=json.load(sys.stdin)
print(f\"Pass rate: {d['pass_rate']:.0%} ({d['tasks_passed']}/{d['tasks_total']})\")
print(f\"Baseline: {d.get('baseline_pass_rate',0):.0%}\")
print(f\"Lift:     {d.get('lift',0):+.0%}\")
"
```

---

## Detailed Setup

### Step 1: Configure IRIS

The benchmark needs IRIS Community Edition. Use Docker — the harness auto-provisions via `iris-devtester`.

```bash
# Option A: Let the harness provision automatically (recommended)
# Just set IRIS_CONTAINER to a name — harness creates it if needed
export IRIS_CONTAINER=iris-bench

# Option B: Use an existing container
docker ps --filter "name=iris" --format "{{.Names}} {{.Ports}}"
export IRIS_CONTAINER=your-existing-container
```

### Step 2: Configure LLM access

The harness supports AWS Bedrock (default) and OpenAI:

```bash
# AWS Bedrock (Claude Sonnet 4.6 — matches published scores)
export AWS_DEFAULT_REGION=us-east-1
# Ensure AWS credentials are configured: aws configure or SSO login
export BENCH_MODEL=us.anthropic.claude-sonnet-4-6

# AWS Bedrock (Claude Opus 4.6 — higher quality, slower)
export BENCH_MODEL=us.anthropic.claude-opus-4-6-v1

# OpenAI (comparable to published gpt-4.1 baseline)
export BENCH_MODEL=gpt-4.1
export OPENAI_API_KEY=sk-...
```

### Step 3: Run the benchmark

```bash
# Basic run — with your skill, no baseline comparison
python3 bench/run_benchmark.py \
  --skill path/to/your/SKILL.md

# With baseline (runs twice — with skill AND without — shows lift)
python3 bench/run_benchmark.py \
  --skill path/to/your/SKILL.md \
  --baseline

# Against a specific benchmark suite
python3 bench/run_benchmark.py \
  --skill path/to/your/SKILL.md \
  --suite jira       # 22-task repair benchmark (default)
  # --suite mf        # 5-task multi-file repair
  # --suite sql       # 14-task IRIS SQL quirks

# Save results to file
python3 bench/run_benchmark.py \
  --skill path/to/your/SKILL.md \
  --baseline \
  --output my_skill_results.json
```

---

## Understanding Results

```
Running 22-task benchmark on iris-bench (IRIS 2025.1)
Skill: my-skill/SKILL.md (247 words)

  [1/22] jira-001 easy   ✓ fixed in iteration 1  (4.1s)
  [2/22] jira-002 easy   ✓ fixed in iteration 1  (3.6s)
  [3/22] jira-003 easy   ✓ fixed in iteration 2  (8.2s)
  ...
  [22/22] jira-056 medium ✓ fixed in iteration 1  (0.7s)

Baseline (no skill):  14/22 = 64%
With skill:           20/22 = 91%
Lift:                 +27%
State:                reviewed  ← automatically set if >= 80%
skill.toml written → results embedded in SKILL.md frontmatter
```

**Interpreting lift:**
- `+15%` or higher → genuinely useful, submit to leaderboard
- `+5% to +15%` → useful for its specific domain, label as domain-specific
- `0% to +5%` → marginal, probably too broad or too narrow
- **Negative lift** → the skill is hurting on tasks where it isn't relevant; load on demand only, not globally

---

## Writing a Skill That Will Score Well

The data is clear: **shorter hard-gate checklists beat long reference documents**.

| Design | Example | Score |
|--------|---------|-------|
| 205-word hard gate checklist | `objectscript-review` | **100%** |
| 268-word all-in-one | `iris-light-slim` | 86% |
| 472-word pattern reference | `objectscript-list-patterns` | 91% |
| 5,170-word comprehensive reference | `iris-light` | 21% |

### The RED-GREEN methodology

**RED**: Run the baseline first (no skill). See which tasks fail and what the model says when you ask why.

```bash
# Get baseline — what fails without any skill?
python3 bench/run_benchmark.py --no-skill --output baseline.json
# Then read what the model generates for failing tasks:
python3 bench/diagnose.py --results baseline.json --task jira-019
```

**GREEN**: Write a skill that addresses the specific failure patterns you observed.

**REFACTOR**: Run benchmark again. If pass rate dropped on some tasks, your skill is too broad — narrow it.

### Skill format

```yaml
---
name: "yourgithub/your-skill-name"
description: "Use when [narrow trigger conditions]"
iris_version: ">=2024.1"
tags: [objectscript]
author: yourgithub
state: draft                    # set to "reviewed" automatically when >= 80%
---

# Your Skill Title

## HARD GATE

Do not show code until this passes.

- [ ] Rule 1
- [ ] Rule 2
...

## Output Format

If violations: > ⚠️ [N] issues found: ...
If clean: > ✅ Passed.
```

### Rules that make skills work

1. **Description = "Use when..." only** — if you summarize the workflow, the model follows the description and skips the body
2. **Hard gate = checkboxes, not prose** — `- [ ] Check X` is read; a paragraph is skimmed
3. **< 300 words for general skills** — models skim long context; your checklist gets ignored
4. **One pattern per skill** — a skill for `$Order` loops is better than one for "all loop patterns"

---

## Submitting to the Leaderboard

### What we accept

- Skills with measured benchmark results (pass rate + baseline + lift)
- Skills that improve on at least one suite
- Skills with a narrow, specific trigger description
- Skills that are self-contained (no external references required)

### What we note but still accept

- Skills with negative lift on the repair suite — labeled "domain-specific, load on demand"
- Skills that score well on SQL/MF but not repair — different suites, different value

### PR format

Open a PR to [intersystems-community/vscode-objectscript-mcp](https://github.com/intersystems-community/vscode-objectscript-mcp) with:

1. Your skill file at `light-skills/skills/yourgithub/your-skill/SKILL.md`
2. PR description including:

```markdown
## Skill: yourgithub/your-skill-name

**Suite**: [repair | mf | sql]
**Pass rate**: XX%
**Baseline**: XX%
**Lift**: +XX%
**IRIS version**: 2025.1
**Model**: claude-sonnet-4-6
**Words**: NNN

### What this catches that other skills don't
[One paragraph]

### Benchmark output
[Paste the summary section from run_benchmark.py output]
```

---

## Running All Suites

```bash
# Run all three suites and get a full comparison
python3 bench/run_all_suites.py \
  --skill path/to/SKILL.md \
  --baseline \
  --output full_results.json

# Compare multiple skills head-to-head
python3 bench/compare_skills.py \
  --skills \
    light-skills/skills/objectscript-review/SKILL.md \
    light-skills/skills/iris-light-slim/SKILL.md \
    path/to/your/SKILL.md \
  --baseline \
  --suite jira
```

---

## Troubleshooting

**`IRIS container not found`**
```bash
docker run -d --name iris-bench -p 1972:1972 \
  intersystemsdc/iris-community:latest
```

**`BenchmarkContainerError: container not running`**
```bash
docker start iris-bench
# Wait 30 seconds, then retry
```

**`LLM authentication failed`**
```bash
# AWS Bedrock:
aws sso login --profile your-profile
# or: aws configure

# OpenAI:
export OPENAI_API_KEY=sk-...
```

**`Benchmark score much lower than published`**
- Verify model: `echo $BENCH_MODEL` — should be `us.anthropic.claude-sonnet-4-6`
- Verify IRIS version: `docker exec iris-bench iris session IRIS 'write $ZVERSION halt'`
- The BenchRunner class may be stale — delete it: `docker exec -i iris-bench iris session IRIS -U USER 'do ##class(%SYSTEM.OBJ).Delete("IrisLight.BenchRunner") halt'`

**`All tasks time out (>30s each)`**
- LLM is slow — try `--task-timeout 60`
- Or switch to a faster model: `BENCH_MODEL=us.anthropic.claude-sonnet-4-6`

---

## Benchmark Task Format

Each task is a JSON file in `bench/eval_tasks/`:

```json
{
  "task_id": "jira-001",
  "difficulty": "easy",
  "description": "Fix null pointer error when processing empty patient records",
  "goal": "Add $IsObject check before accessing object properties",
  "initial_code": {
    "files": [{"path": "src/X.cls", "content": "...buggy code..."}]
  },
  "test_code": {"path": "tests/TestX.cls", "content": "...test that fails on bug..."},
  "hints": [],
  "expected_behavior": "..."
}
```

**Adding new tasks**: See `bench/eval_tasks/README.md`. Tasks must:
1. Compile on buggy code (syntax errors are a different skill test)
2. Fail the test on buggy code
3. Pass the test on the correct fix
4. Be self-contained (no external class dependencies)

Current suites:
- `bench/eval_tasks/jira_bugs/` — 22 single-function repair tasks
- `bench/eval_tasks/mf_bugs/` — 5 multi-file repair tasks  
- `bench/eval_tasks/iris_sql/` — 14 IRIS SQL quirks tasks

---

## Questions?

File issues at [intersystems-community/vscode-objectscript-mcp](https://github.com/intersystems-community/vscode-objectscript-mcp) or ping `@tdyar` / `@tleavitt` on Teams.
