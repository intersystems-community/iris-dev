---
name: objectscript-debugging
description: Captures IRIS diagnostic packets, maps .INT offsets to .CLS source lines, and correlates error logs. Use whenever an IRIS runtime error or compile failure needs to be diagnosed.
license: MIT
metadata:
  version: "1.0.0"
  author: InterSystems Developer Community
  compatibility: objectscript, iris, healthconnect
  spec: 011-debugging-analysis
---

## Purpose
Ground AI debugging in real IRIS signals — compiler errors, `messages.log` entries, stack traces, and local variable state — rather than guessing from code alone.

## When to Use
- Any `<UNDEFINED>`, `<SUBSCRIPT>`, `<PROTECT>`, or other IRIS runtime error
- Compile failures where the error message references a `.INT` offset (e.g. `+3^MyApp.Foo.1`)
- Debugging a Business Operation or Production component that is failing silently
- Correlating multiple error log entries to a single root cause

## Workflow

### Step 1 — Capture Diagnostic Packet
Call `debug_capture_packet` to snapshot the current IRIS error state:
```
Tool: debug_capture_packet
Inputs: namespace (default USER), include_locals (default true)
Returns: error_message, stack trace, redacted local variables, timestamp
```
Always capture before attempting a fix — the packet is your ground truth.

### Step 2 — Map .INT Offset to .CLS Source
If the error references a `.INT` routine offset, resolve it to the original `.CLS` source line:
```
Tool: debug_map_int_to_cls
Inputs: error_string (e.g. "<UNDEFINED>x+3^MyApp.Foo.1") OR (routine, offset)
Returns: class name, method name, line number, source snippet
```
You can pass the raw IRIS error string directly — the tool parses it automatically.

### Step 3 — Read Error Logs
Pull recent errors from `messages.log` and `^ERRORS` global:
```
Tool: objectscript_debug_get_error_logs
Inputs: time_window_hours (default 24), max_entries (default 100)
Returns: unified log with timestamps, error codes, namespaces
```

### Step 4 — Correlate and Fix
Cross-reference the packet, source mapping, and logs to identify root cause. Apply fix. Recompile. Verify no new errors appear in logs.

## Common Error Patterns

| Error | Likely cause | Fix pattern |
|---|---|---|
| `<UNDEFINED>` | Variable used before SET | Check method entry path; add default |
| `<SUBSCRIPT>` | Global subscript too long | Truncate or hash the key |
| `<PROTECT>` | Privilege error on global | Check resource definitions |
| `ERROR #5002` | ObjectScript compilation error | Read compiler output line by line |
| `ERROR #5659` | Method signature mismatch | Check superclass method signature |

## Output Format

After diagnosis:
> 🔍 IRIS diagnostic packet captured: `<packet-id>`
> **Error**: `<error_message>`
> **Source**: `<ClassName>:<MethodName>` line `<N>`
> **Root cause**: [explanation]
> **Fix**: [proposed change]
