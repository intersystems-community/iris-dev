---
name: ensemble-production
description: Manage and observe IRIS Interoperability productions — lifecycle, logs, queues, and message tracing
trigger: When asked about a production status, to start/stop/restart a production, investigate message failures, or check queue backlogs
---

## Purpose
Operate IRIS Interoperability (Ensemble) productions safely: check status before touching
anything, use targeted tools for each operation type, and always verify after a change.

## Process Flow

### Investigating a production problem

1. **Check status first** — call `interop_production_status` with `full_status=true`
   to see which components are running, faulted, or disabled.

2. **Check queues** — call `interop_queues` if you suspect backlog or blocked messages.
   High queue depth on a specific component indicates a bottleneck or fault in that component.

3. **Search messages** — call `interop_message_search` to find specific messages by body
   content, session ID, sender, or time range. This is the fastest way to trace a failed
   transaction end-to-end.

4. **Check logs** — call `interop_logs` filtered to the component and time window of interest.
   Look for `ERROR` or `WARNING` severity entries.

### Making a configuration change

1. Call `interop_production_needs_update` — if it returns `false`, no action needed.
2. If update needed, call `interop_production_update` (hot-apply, no downtime).
3. Confirm with `interop_production_status` that all components are still running.

### Restarting a production

Only restart if status shows the production is stopped or stuck:

```
# Graceful stop (waits for in-flight messages)
interop_production_stop(timeout=30, force=false)

# Start with the production class name
interop_production_start(production="MyApp.Productions.Main", namespace="MYNS")

# Confirm
interop_production_status(full_status=true)
```

### Recovering a faulted production

If the production is in an error state (stuck, partially started), call `interop_production_recover`.
This performs the equivalent of the Management Portal "Recover" button.

## Tool Reference

| Tool | When to use |
|------|------------|
| `interop_production_status` | Always first — baseline state before any action |
| `interop_production_start` | Start a stopped production |
| `interop_production_stop` | Graceful or forced stop |
| `interop_production_update` | Hot-apply config changes (no restart needed) |
| `interop_production_needs_update` | Check before deciding whether to update |
| `interop_production_recover` | Un-stick a faulted/partially-started production |
| `interop_logs` | Component-level log entries (filter by component + severity) |
| `interop_queues` | Queue depth per component — spot bottlenecks |
| `interop_message_search` | Trace specific messages by content, session, or time |

## Safety Rules

- **Never force-stop** (`force=true`) unless graceful stop has timed out. Force-stop drops
  in-flight messages.
- **Always check `interop_production_needs_update` before `interop_production_update`** — calling
  update when not needed is a no-op, but it's good hygiene to confirm first.
- **Namespace matters** — every tool accepts a `namespace` parameter. Default is `USER`.
  Productions in `HSCUSTOM` or application-specific namespaces require the correct namespace.
- **Do not restart to fix a config change** — use `update` instead. Restart loses in-flight messages.

## Output Format

When reporting production state:
> **Production**: `MyApp.Productions.Main` — RUNNING
> **Components**: 12 running, 0 faulted, 2 disabled
> **Queue depth**: BusinessProcess.OrderHandler: 0, BusinessOperation.SendHL7: 3

When tracing a message failure:
> **Session** `12345`: Failed at `BusinessOperation.SendHL7` — ERROR: Connection refused
> **Fix**: Check the outbound adapter host/port configuration for SendHL7
