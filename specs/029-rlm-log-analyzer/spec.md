# Feature Specification: RLM-Based Log Analyzer

**Feature Branch**: `029-rlm-log-analyzer` (not yet created)
**Created**: 2026-05-02
**Status**: Sketch — not ready to implement
**Closes**: GitHub issue #24

## Overview

`iris_analyze_logs` is a new tool that uses the RLM (Recursive Language Model) pattern to
analyze IRIS error log entries without flooding context. Rather than dumping all entries
into a prompt, the agent writes code that iterates over the UUID log store (from feature 027),
clusters entries by error code and class, and builds up a structured diagnosis in variables.

The key insight from the RLM literature (Zhang et al., Khattab/DSPy): context lives in
variables, not in prompts. The log store's `iris_get_log` with `limit`/`offset` is the
exact primitive an RLM needs to page through results programmatically. No MCP sampling
support required — works with any client.

## The Approach

Instead of:
```
iris_analyze_logs → dumps 500 entries into prompt → LLM analyzes
```

Do:
```
iris_analyze_logs →
  for each page of log_store entries:
    count by error_code
    identify most-affected classes
    detect temporal clusters (bursts)
  → return structured summary: {root_cause_candidates, affected_classes, timeline, suggested_next_steps}
```

The analysis is deterministic Rust code for the structural parts (counting, clustering,
timeline). The LLM is only invoked for the narrative synthesis at the end — and only when
`detail=true` is requested.

## Sketch: Tool Interface

```
iris_analyze_logs(
  namespace: string,          // which IRIS namespace to query
  log_id: Option<string>,     // analyze a specific stored log (from iris_get_log)
  limit: usize,               // max entries to analyze (default 200)
  detail: bool,               // false = structural summary only; true = narrative synthesis
  focus: Option<string>,      // class/package prefix to filter on
)
```

Returns (detail=false):
```json
{
  "success": true,
  "entry_count": 847,
  "time_range": {"first": "...", "last": "..."},
  "top_error_codes": [{"code": "UNDEFINED", "count": 312}, ...],
  "most_affected_classes": [{"class": "MyApp.Service", "count": 89}, ...],
  "burst_detected": true,
  "burst_window": {"start": "...", "end": "...", "count": 412},
  "suggested_focus": "MyApp.Service"
}
```

Returns (detail=true): adds `narrative` field with root cause analysis synthesized from
the structural summary (short LLM call, not a raw log dump).

## RLM Pattern Notes

- The `iris_get_log` store (feature 027) provides UUID-keyed paginated access — this is the
  RLM "external memory" that the analyzer iterates over without touching context.
- For live log analysis (not from store), the analyzer uses `debug_get_error_logs` with
  pagination via `IRIS_INLINE_ERROR_LOGS` threshold.
- The structural analysis (counts, clusters, timeline) runs in Rust — no LLM call for
  `detail=false`. This is the cheap default.
- `detail=true` generates a compact synthesis prompt from the structural result (not the
  raw entries) and calls the agent's LLM if MCP sampling is available, or returns the
  structural result with a `synthesis_unavailable` note if not.

## Dependencies

- Feature 027 (log store + iris_get_log) — REQUIRED, already shipped
- MCP sampling (rmcp `create_message`) — OPTIONAL, for `detail=true` only

## Open Questions (not resolved)

- Should the RLM iteration happen client-side (agent writes code) or server-side (Rust
  loops over the store)? Server-side is simpler but less "RLM-pure". Client-side requires
  the agent to know about `iris_get_log` pagination.
- What's the right `burst_detection` threshold? Fixed window? Adaptive?
- Should the tool write a persistent analysis result back to the log store for later retrieval?
