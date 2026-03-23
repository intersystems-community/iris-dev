# Research: 017-interop-tools

## Decision 1: Atelier REST mapping for team-23 IRIS calls

### Findings

Team-23 uses three patterns:
1. `iris.classMethodString("Class","Method",args)` → **xecute**: `POST /api/atelier/v1/{ns}/action/xecute`
2. `iris.classMethodObject("Class","%OpenId",id)` → **xecute** with Write of object properties
3. SQL cursor queries → **query**: `POST /api/atelier/v1/{ns}/action/query`

All 15 tools map cleanly to xecute or query. No new connection infrastructure needed.

### Key mappings

| team-23 call | Atelier REST equivalent |
|---|---|
| `iris.classMethodString("Ens.Director","GetProductionStatus",.name,.state)` | xecute: `Set sc=##class(Ens.Director).GetProductionStatus(.name,.state) Write name_":"_state` |
| `iris.classMethodString("Ens.Director","StartProduction",name)` | xecute: `Do ##class(Ens.Director).StartProduction("name")` |
| SQL `SELECT * FROM Ens_Util.Log` | query: same SQL |
| SQL `SELECT * FROM Ens.Queue_Enumerate()` | query: same SQL |

### Complication: ByRef parameters

`GetProductionStatus` uses ByRef params (.name, .state). The xecute endpoint doesn't support ByRef return. Two options:
1. **Wrap in ObjectScript that Writes the results**: `Set sc=##class(Ens.Director).GetProductionStatus(.n,.s) Write {"production":(n),"state":(s)}`
2. Use SQL: `SELECT Name,Status FROM Ens_Config.Production WHERE ID=1`

Decision: Option 1 — write an inline ObjectScript snippet that calls the class method and Writes JSON. This is the pattern already used by `iris_compile` and `debug_map_int_to_cls`.

## Decision 2: Production state values

`Ens.Director.GetProductionStatus` returns integer state:
- 1 = Running
- 2 = Stopped
- 3 = Suspended
- 4 = Troubled
- 5 = NetworkStopped

Map to human-readable strings in the tool response.

## Decision 3: Error handling pattern

All tools follow the existing iris-dev pattern:
- Network error → `error_code: "IRIS_UNREACHABLE"`
- IRIS returns error status → `error_code: "INTEROP_ERROR"` with message
- No production running → `error_code: "NO_PRODUCTION"` with empty state
- Success → `success: true` + structured data

## Decision 4: Tool naming convention

Prefix: `interop_` (consistent with spec). Names:
- `interop_production_status`
- `interop_production_start`
- `interop_production_stop`
- `interop_production_update`
- `interop_production_needs_update`
- `interop_production_recover`
- `interop_logs`
- `interop_queues`
- `interop_message_search`

All underscore-only (Bedrock/VS Code compatible).
