# Demo Script: 017 — Interop Tools

## Verified Results (iris-dev-iris, 2026-03-22)

| Tool | Result | Latency |
|---|---|---|
| interop_production_status | NO_PRODUCTION (correct — no prod running) | 37ms |
| interop_logs | OK (Ens_Util.Log accessible) | 204ms |
| interop_queues | OK (Ens.Queue_Enumerate accessible) | 11ms |
| interop_message_search | OK (Ens.MessageHeader accessible) | 218ms |
| interop_production_needs_update | OK (boolean response) | 13ms |
| interop_production_recover | INTEROP_ERROR (nothing to recover) | 10ms |

**SC-003: All under 2s** — max 218ms.
**SC-001: 32 tools** — verified.
**SC-002: All structured JSON** — verified.

## Demo Flow

```bash
iris-dev mcp --subscribe intersystems-community/vscode-objectscript-mcp
```

In VS Code Copilot agent mode:

1. "What's the production status?" → `interop_production_status` → shows NO_PRODUCTION
2. "Show me recent error logs" → `interop_logs` → structured log entries
3. "Are there any backed up queues?" → `interop_queues` → queue depths
4. "Search for recent messages" → `interop_message_search` → message archive

For live production demo: need to create and start a production first via `interop_production_start`.

## Notes

- iris-dev-iris (Community IRIS) has Ensemble classes available
- No production is pre-configured — create one for the full lifecycle demo
- All tools gracefully handle missing/stopped productions
