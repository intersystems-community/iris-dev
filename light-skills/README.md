# ObjectScript AI — Light Skills

**Zero-infrastructure AI coding support for ObjectScript / IRIS.**

No MCP server. No Python packages. No pip installs. Just `curl`, an `AGENTS.md`, and two skill files.

---

## What's in here

| File | Purpose |
|---|---|
| `AGENTS.md` | Drop into your repo root — teaches AI coding agents ObjectScript semantics, gotchas, and the compile/test loop |
| `introspect.md` | Skill: fetch any class's full source from IRIS via Atelier REST before writing code that touches it |
| `compile.md` | Skill: upload a `.cls` file, compile it, and get structured error output for the AI to fix |

---

## Quick start

### 1. Copy AGENTS.md to your repo

```bash
cp light-skills/AGENTS.md /path/to/your-project/AGENTS.md
# or for Claude Code:
cp light-skills/AGENTS.md /path/to/your-project/.claude/AGENTS.md
```

That's it. Your AI agent will now follow ObjectScript rules automatically.

### 2. Install the skills (optional but high-value)

#### Claude Code
```bash
mkdir -p /path/to/your-project/.claude/skills
cp light-skills/introspect.md /path/to/your-project/.claude/skills/
cp light-skills/compile.md    /path/to/your-project/.claude/skills/
```

#### opencode
```bash
mkdir -p /path/to/your-project/.opencode/skills
cp light-skills/introspect.md /path/to/your-project/.opencode/skills/
cp light-skills/compile.md    /path/to/your-project/.opencode/skills/
```

#### Global install (all your projects)
```bash
# Claude Code / opencode global skills (single source of truth: vscode-objectscript-mcp)
mkdir -p ~/.claude/skills
for skill in iris-objectscript-eval objectscript-debugging objectscript-navigation \
             objectscript-repair objectscript-review objectscript-tdd objectscript-unit-test \
             ensemble-production; do
  cp -r ~/ws/vscode-objectscript-mcp/light-skills/skills/$skill ~/.claude/skills/
done

# Codex global skills
mkdir -p ~/.codex/skills
for skill in iris-objectscript-eval objectscript-debugging objectscript-navigation \
             objectscript-repair objectscript-review objectscript-tdd objectscript-unit-test \
             ensemble-production; do
  cp -r ~/ws/vscode-objectscript-mcp/light-skills/skills/$skill ~/.codex/skills/
done
```

> **Canonical source**: `~/ws/vscode-objectscript-mcp/light-skills/skills/` is the single source of truth for all ObjectScript agent skills. Do not edit skills in `objectscript-coder/light-skills/skills/` directly — update in `vscode-objectscript-mcp` and re-copy.

### 3. Set your IRIS connection env vars

```bash
export IRIS_HOST=localhost      # or your Docker host / remote server
export IRIS_WEB_PORT=52773      # Atelier web port (NOT 1972)
export IRIS_USER=_SYSTEM
export IRIS_PASS=SYS
export IRIS_NS=USER             # your working namespace
```

> **Docker tip**: `docker port <container> 52773` tells you the mapped host port.

### 4. Use the skills

```
/introspect EnsLib.HTTP.OutboundAdapter
/compile MyPackage.MyService src/MyPackage/MyService.cls
```

---

## Why this works

Based on internal testing with Claude Sonnet 4.6 and o4-mini:

- **Without** any tools: 0% success on custom app classes and private server-side classes
- **With** `/introspect` before coding: 100% success on the same tasks

The AI doesn't hallucinate method signatures when it has the actual source. `AGENTS.md` prevents
the most common ObjectScript mistakes (`%TimeStamp` format, `Quit` vs `Return`, `..` method calls)
before they happen.

---

## What AGENTS.md covers

- `Quit` vs `Return` rules
- `%TimeStamp` format gotcha (`YYYY-MM-DD HH:MM:SS`, not ISO 8601 with `T`)
- `%Status` return convention and macros
- Error handling patterns (`$$$ThrowOnError`, `Try/Catch`)
- Transaction discipline (`$TLEVEL` check before rollback)
- Intra-class method call syntax (`..Method()`)
- Operator precedence (none — left-to-right only)
- Namespace awareness
- Compile error interpretation guide
- Class structure templates (registered object, persistent, unit test)

---

## Want more?

The full `iris-dev mcp` server adds:
- `docs_introspect` — same as `/introspect` but invoked automatically by the AI, no manual `/invoke` needed
- `iris_symbols_local` — parses `.cls` files on disk without a running IRIS (tree-sitter based)
- `kb_index` / `kb_recall` — vector+BM25 knowledge base over your own codebase
- Learning agent — synthesizes skills from your session history

See the repo README for setup: `iris-dev mcp`
