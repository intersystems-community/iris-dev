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

## Decision 5: team-17 integration (Riyadh hackathon — TrakCare Root Cause Analysis)

### Findings

Team-17 (`~/ws/team-17/`) has a working diagnostic assistant with real indexed data:
- **271 TrakCare .cls XML files** → 445 sourcecode embeddings (BGE 1024-dim, NumPy)
- **1,450 Jira issue embeddings** (NumPy)
- **14,749 ticket embeddings** (NumPy)
- FastAPI server with `DiagnosticAgent` orchestrating 4 search services
- `run_demo_scenarios.py` for scripted demos

### What this means for 017

The demo strategy shifts from "build a production from scratch" to "two agents coordinate through IRIS":

**Agent 1**: team-17 diagnostic assistant (searches tickets + Jira + docs + ObjectScript source)
**Agent 2**: kg-ticket-resolver (IRIS KG + GraphRAG) 
**Coordination**: agent bus findings in IRIS

The `interop_*` tools let Agent 2 monitor the production that feeds Agent 1.

### Data migration opportunity

Team-17's NumPy embeddings (BGE 1024-dim) → IRIS VECTOR(FLOAT, 1024) via `langchain-intersystems IRISVectorStore`. This ports their search index into IRIS natively, eliminating the NumPy dependency.

### Not in scope for 017

The actual team-17 FastAPI migration is separate work. Spec 017 delivers the interop tools. The demo wiring (agent bus, two-agent coordination) is Phase 3 rehearsal work.
