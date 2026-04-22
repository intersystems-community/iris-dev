# Research — Spec 021 Path-Aware Benchmark

## R1: LLM-as-Judge Rubric Design

Decision: Use Anthropic Claude Haiku 4.5 as the judge model via the Claude API. Fixed rubric prompt injected per task with: task description, expected behavior, agent transcript (tool calls + final output), and the 0-3 scoring criteria. Judge returns JSON: `{"score": N, "reasoning": "..."}`.

Rationale: Haiku is fast and cheap enough to score every task without budget pressure. Fixed rubric ensures reproducibility — same prompt, same model, same score for same input. Claude is also familiar with ObjectScript which reduces hallucinated judgments about correctness.

Alternatives considered:
- GPT-4o as judge: works but adds an external dependency and cost
- Human scoring: too slow for iterative benchmark runs
- Pure assertion: too brittle for open-ended GEN/MOD/LEG tasks

Rubric prompt structure:
```
You are evaluating an AI coding agent's performance on an ObjectScript task.

TASK: {task.description}
EXPECTED: {task.expected_behavior}
PATH: {path_a|path_b}

AGENT TRANSCRIPT:
{tool_calls_and_output}

Score 0-3:
0 = Failed or wrong output
1 = Compiled but incorrect behavior
2 = Correct but excessive tool calls (>2 unnecessary)
3 = Correct and efficient

Return JSON: {"score": N, "reasoning": "one sentence"}
```

## R2: Claude Code Harness (MCP stdio)

Decision: Drive Claude Code via the Anthropic API directly (not via the CLI), using the claude-sdk with tools injected as the iris-dev MCP server tools. This gives full programmatic control — submit prompt, collect tool calls, collect final response, score.

Rationale: The claude-dev CLI is not scriptable in a way that captures tool call traces. The Anthropic API with tool_use gives us the full transcript needed for scoring (tool name, args, result, final output).

Implementation: Python benchmark runner calls `anthropic.messages.create()` with:
- System prompt declaring the active path (Path A or Path B)
- Task prompt
- Tools array from iris-dev MCP server (discovered via initialize + tools/list)
- Loop: execute tool calls against live iris-dev mcp process, feed results back

## R3: Copilot Harness Automation

Decision: Do NOT use Playwright to drive VS Code Copilot Chat. Use the VS Code Extension Test Host API (`vscode-test` + `vscode.commands.executeCommand`) instead.

Rationale: Copilot Chat in VS Code 1.99+ uses WebView/Canvas rendering. The chat input has no stable DOM selectors — Playwright locators fail reliably against shadow-DOM nested or Canvas-rendered elements. Standard AI coding benchmarks (SWE-bench, Aider) do not use IDE UI automation; they drive agents via direct API or CLI.

Correct approach for Copilot harness:
- Launch VS Code Extension Test Host via `@vscode/test-electron`
- Submit prompts via `vscode.commands.executeCommand('github.copilot.sendToChat', prompt)`
- Poll for response completion via extension API (no streaming wait needed)
- Capture tool call list from iris-dev MCP server's stdio transcript (the runner already has this)

Alternatives considered:
- Playwright DOM automation: fails — WebView/Canvas has no stable selectors
- Direct Copilot API: not publicly available for agent mode
- Manual Copilot testing: valid for initial validation, not for repeatable benchmark

Impact on plan: Phase 3 (T015-T018) changes from "Playwright" to "vscode-test-electron". The driver (`copilot.py`) becomes a Node.js/TypeScript extension test script rather than Python Playwright. Python runner orchestrates it via subprocess.

## R4: Task Isolation — BENCHMARK Namespace

Decision: Dedicated `BENCHMARK` namespace in iris-dev-iris container. Wiped between tasks using:
```objectscript
kill ^||%SYS.Namespace("BENCHMARK")  // or via iris_execute: kill ^*
```
More precisely: use Atelier xecute to run `kill ^AgentEval.*` scoped to BENCHMARK namespace, then re-create any fixture globals needed for the task.

Namespace creation: `iris_execute` with `do ##class(%SYS.Namespace).Create("BENCHMARK")` on first run if not exists.

## R5: Results Schema

Decision: JSON schema for each run result:

```json
{
  "run_id": "2026-04-22T14:00:00Z",
  "iris_dev_version": "0.2.0",
  "tasks": [
    {
      "id": "GEN-01",
      "category": "GEN",
      "path": "A",
      "harness": "claude-code",
      "score": 3,
      "reasoning": "Class created correctly, compiled first try, 2 tool calls",
      "tool_calls": ["iris_compile", "iris_execute"],
      "tool_call_count": 2,
      "scm_elicitation_triggered": false,
      "duration_ms": 4200
    }
  ],
  "summary": {
    "mean_score_path_a": 2.6,
    "mean_score_path_b": 1.4,
    "by_category": {}
  }
}
```

HTML report generated from this JSON using a self-contained template (visual-explainer style) — bar charts per category, path comparison table, shareable without a server.
