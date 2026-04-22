# iris-dev Early Access Program

## Overview

iris-dev is a single-binary MCP (Model Context Protocol) server that gives AI coding assistants — GitHub Copilot, Claude Code, Cursor, and others — a native interface to InterSystems IRIS. Developers install one file, point it at their IRIS instance, and their AI assistant can compile, execute, test, search, introspect, and debug ObjectScript directly against live IRIS with no Python, no Node, and no pip conflicts.

A built-in skills system observes agent sessions, synthesizes reusable ObjectScript workflows, and improves its own behavior over time. Skills can be shared across the community, giving every developer the benefit of patterns learned by others.

iris-dev ships with a path-aware agentic benchmark (23 tasks across 5 categories: code generation, modification, debugging, source control, and legacy .mac/globals patterns) that scores AI agent performance on two development paths: local files + Atelier (the recommended path) vs. ISFS-only (legacy/remote). The benchmark is repeatable, LLM-judged, and produces a shareable HTML report. EAP feedback will directly inform which path performs better in real-world conditions and what tooling gaps exist.

**GitHub:** https://github.com/intersystems-community/iris-dev

## Goals

- Validate that the single-binary distribution model works for Windows, macOS, and Linux users
- Identify gaps in tool coverage — what operations do agents attempt that iris-dev doesn't support?
- Measure quality of agent-generated ObjectScript across development paths (local files vs. ISFS-only) using the built-in benchmark
- Collect feedback on the VS Code extension and MCP server configuration experience
- Gather input on the skills system — are learned workflows useful, accurate, and shareable?
- Build empirical evidence for the recommended development path before public announcement

## What Participants Get

- iris-dev binary and VS Code extension — all on the public GitHub Releases page: https://github.com/intersystems-community/iris-dev/releases/latest
- Access to the path-aware benchmark — run it against your own codebase to see how your setup scores
- Direct access to the development team via GitHub Issues
- Influence over the roadmap before GA

## What We're Looking For

- Developers actively using Copilot or Claude Code for ObjectScript work
- Ideally: a mix of local-file workspaces (modern path) and ISFS/remote-only setups (legacy path)
- Willingness to file GitHub issues with specific failure cases
- Optional: run the benchmark against your setup and share the `scores.json` — your data helps calibrate the path comparison
- Optional: participation in a 30-minute debrief session after 2-4 weeks of use

## Setup (3 steps)

1. Go to https://github.com/intersystems-community/iris-dev/releases/latest and download the binary for your platform. Put it on your PATH (macOS/Linux: `mv iris-dev-* /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev`; Windows: rename to `iris-dev.exe`, place on PATH or set `iris-dev.serverPath` in VS Code settings).
2. Download `vscode-iris-dev-*.vsix` from the same release page and install in VS Code 1.99+ via **Extensions: Install from VSIX**.
3. Ensure `objectscript.conn` is configured in VS Code settings — iris-dev reads it automatically.

That's it. No Python, no pip, no node_modules.

## Running the Benchmark (optional but encouraged)

The benchmark ships with iris-dev and measures how well AI agents perform ObjectScript tasks on your setup:

```bash
export IRIS_HOST=localhost IRIS_WEB_PORT=52773
cd iris-dev
python -m benchmark.021.runner --dry-run              # preview tasks
python -m benchmark.021.runner --path A --categories GEN   # run GEN tasks, local-file path
```

Results are written to `benchmark/021/results/` as `scores.json` and a visual `report.html`. Share `scores.json` with us to contribute to the aggregate path comparison dataset.

## Feedback Channels

- **GitHub Issues:** https://github.com/intersystems-community/iris-dev/issues (preferred for bugs and feature requests)
- **Email:** thomas.dyar@intersystems.com

## Timeline

- EAP open: April 2026 (at READY conference)
- Target close / public GA: TBD based on feedback

## Participants

| Name | Company | Setup | Enrolled | Notes |
|------|---------|-------|----------|-------|
| | | | | |

## Benchmark Results Collected

| Metric | Value |
|--------|-------|
| Path A mean score (local files + Atelier) | — |
| Path B mean score (ISFS only) | — |
| Participants contributing benchmark data | 0 |

## Results / What We Learned

*(fill in post-EAP)*
