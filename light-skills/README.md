# ObjectScript AI — Light Skills

**Benchmark-validated AI coding support for ObjectScript / IRIS.**

No MCP server. No Python. No pip installs. Copy two files, get measurably better AI-generated ObjectScript.

---

## The numbers

Tested with Claude Sonnet 4.6 on a 22-task ObjectScript repair benchmark (real bug patterns from ISC internal codebases):

| What you add | Pass rate | Lift |
|---|---|---|
| Nothing (baseline) | 73% | — |
| `AGENTS.md` only | 86% | +14% |
| `AGENTS.md` + `objectscript-review` skill | **100%** | **+29%** |

The benchmark covers: null pointer bugs, SQL filter errors, loop logic, HTML escaping, type conversion, date validation, list operations — the real mistakes AI makes when writing ObjectScript without context.

---

## 60-second setup

### Step 1: Copy AGENTS.md to your repo

```bash
curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/AGENTS.md \
  > AGENTS.md
```

That's it for the baseline (+14%). Your AI agent now knows the top 10 ObjectScript gotchas.

### Step 2: Add the review skill for 100% (recommended)

**Claude Code:**
```bash
mkdir -p .claude/skills/objectscript-review
curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/skills/objectscript-review/SKILL.md \
  > .claude/skills/objectscript-review/SKILL.md
```

**opencode:**
```bash
mkdir -p ~/.config/opencode/skills/objectscript-review
curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/skills/objectscript-review/SKILL.md \
  > ~/.config/opencode/skills/objectscript-review/SKILL.md
```

The review skill is a **hard gate** — it runs before showing you any ObjectScript code and corrects mistakes before they reach you.

### Step 3 (optional): Add more validated skills

| Skill | Pass rate | Best for |
|---|---|---|
| `objectscript-review` | **100%** | Hard-gate review — the anchor skill |
| `objectscript-list-patterns` | 91% | `%List`, CSV building, backwards iteration |
| `objectscript-unit-test` | 86% | Generating `%UnitTest.TestCase` scaffolds |
| `objectscript-navigation` | 82% | Codebase discovery, class browsing |
| `objectscript-sql-patterns` | ✓ | SQL table naming, `$HOROLOG` dates, SQLCODE |
| `objectscript-loop-patterns` | ✓ | `$Order`, `Return` vs `Quit` in loops |

```bash
# Install all validated skills (global — works for all your projects)
SKILLS_DIR=~/.config/opencode/skills   # or ~/.claude/skills for Claude Code
BASE=https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/skills

for skill in objectscript-review objectscript-list-patterns objectscript-sql-patterns \
             objectscript-loop-patterns objectscript-unit-test objectscript-navigation; do
  mkdir -p "$SKILLS_DIR/$skill"
  curl -sL "$BASE/$skill/SKILL.md" > "$SKILLS_DIR/$skill/SKILL.md"
done
echo "Done. $(ls $SKILLS_DIR | wc -l) skills installed."
```

---

## What's in this directory

| File/Directory | Purpose |
|---|---|
| `AGENTS.md` | ObjectScript rules — drop in your repo root |
| `compile.md` | Skill: compile via Atelier REST, structured errors |
| `introspect.md` | Skill: fetch any class definition from IRIS |
| `skills/objectscript-review/` | **Start here** — hard-gate review, 100% pass rate |
| `skills/objectscript-*` | Validated pattern skills (see table above) |
| `skills/iris-light-slim/` | 268-word all-in-one hard gate (alternative to review) |
| `kb/` | Reference knowledge: error codes, idioms, IPM authoring |
| `iris-dev.toml` | Package manifest for `iris-dev` CLI install |

---

## For ISC SEs and developers — dogfood instructions

**5 minutes to set up, measurable improvement immediately.**

### If you use Claude Code

```bash
# 1. Copy AGENTS.md to your project
curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/AGENTS.md > AGENTS.md

# 2. Install the top 3 skills globally
mkdir -p ~/.claude/skills
for skill in objectscript-review objectscript-sql-patterns objectscript-loop-patterns; do
  mkdir -p ~/.claude/skills/$skill
  curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/skills/$skill/SKILL.md \
    > ~/.claude/skills/$skill/SKILL.md
done

# 3. Verify
ls ~/.claude/skills/
```

### If you use opencode

```bash
# 1. Copy AGENTS.md to your project
curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/AGENTS.md > AGENTS.md

# 2. Install skills globally
mkdir -p ~/.config/opencode/skills
for skill in objectscript-review objectscript-sql-patterns objectscript-loop-patterns; do
  mkdir -p ~/.config/opencode/skills/$skill
  curl -sL https://raw.githubusercontent.com/intersystems-community/vscode-objectscript-mcp/master/light-skills/skills/$skill/SKILL.md \
    > ~/.config/opencode/skills/$skill/SKILL.md
done
```

### If you use VS Code Copilot

The [VS Code ObjectScript MCP extension](https://github.com/intersystems-community/vscode-objectscript-mcp) wires these skills into Copilot agent mode automatically. Install the extension and it picks up your IRIS connection from VS Code settings.

### Connect to IRIS (for compile/introspect skills)

```bash
export IRIS_HOST=localhost
export IRIS_WEB_PORT=52773     # Atelier REST port — NOT 1972
export IRIS_USER=_SYSTEM
export IRIS_PASS=SYS
export IRIS_NS=USER
```

> **Docker?** `docker port <container> 52773` gives you the mapped port.

---

## What to try first

1. **Open an existing ObjectScript class** in your editor
2. Ask your AI: *"Review this method for ObjectScript mistakes"*
3. With `objectscript-review` loaded, the AI will run the 10-item checklist and correct issues before showing you anything
4. Ask it to *"write a unit test for this class"* — with `objectscript-unit-test`, it reads the actual IRIS class definition first

**Tell us what you find.** File issues at [intersystems-community/vscode-objectscript-mcp](https://github.com/intersystems-community/vscode-objectscript-mcp) or ping `@tdyar` / `@tleavitt` on Teams.

---

## Want the full stack?

The `objectscript-mcp` server adds live IRIS integration — automatic introspection, symbol search, and a learning agent that synthesizes new skills from your session patterns.

```bash
pip install objectscript-mcp
objectscript-mcp  # starts MCP server on stdio
```

Configure in Claude Desktop / opencode `config.json`:
```json
{
  "mcpServers": {
    "objectscript": {
      "command": "objectscript-mcp",
      "env": {
        "IRIS_HOST": "localhost",
        "IRIS_PORT": "1972",
        "IRIS_NAMESPACE": "USER"
      }
    }
  }
}
```

See [MCP_SETUP_GUIDE.md](../docs/MCP_SETUP_GUIDE.md) for full configuration options.

---

## Benchmark methodology

The 22-task repair suite covers real ObjectScript bug patterns. Each task has a buggy `.cls` file, a test that fails on the bug, and an oracle that verifies the fix. Tasks were drawn from internal JIRA bugs and ISC codebase patterns.

All scores measured with Claude Sonnet 4.6 via AWS Bedrock. Lift = skill pass rate − baseline. Tests run on IRIS 2025.1 Community Edition in Docker.

Full benchmark harness: [`objectscript-coder/bench/`](https://github.com/intersystems-community/vscode-objectscript-mcp)
