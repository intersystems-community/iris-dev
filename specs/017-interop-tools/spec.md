# Spec 017 — Interop Tool Group for iris-dev

**Feature branch**: `017-interop-tools`  
**Created**: 2026-03-22  
**Status**: Draft  
**Owner**: Thomas Dyar  
**Demo target**: AIML75 "Multi-Agent Orchestration for Enterprise Workflows" (READY 2026, April 27)

---

## Overview

Add an `interop_*` tool group to `iris-dev` that exposes IRIS Interoperability
Productions — business services, processes, and operations — as MCP tools that
any AI agent can invoke. An agent outside IRIS can build a production, start it,
monitor its health, read error logs, drain queues, and post findings to the agent
bus when something goes wrong.

Two connection paths are implemented and kept in sync:

| Path | Transport | IRIS version required | When to use |
|---|---|---|---|
| **Path 1: Atelier REST** | HTTP → `%api.atelier` | Any IRIS with web server | Default; works everywhere iris-dev already works |
| **Path 2: %AI.ToolSet** | wgproto → `iris-mcp-server` → `%AI.Interop.ToolSet` | New IRIS build with `%AI.*` classes | When aicore is available; adds RAG discovery, policy layer |

Path 1 is the implementation reference. Path 2 is a thin ObjectScript wrapper
that re-exposes the same logical tools through Dave's `%AI.ToolSet` pattern,
enabling `iris-mcp-server` RAG-based tool discovery and `%AI.Policy` enforcement.

---

## Motivation

### The Demo Anchor: kg-ticket-resolver

`~/ws/kg-ticket-resolver-recovered` is a working multi-agent support ticket
resolver that uses IRIS as its KG and iService as its ticket source. It has
an orchestration layer, clustering pipeline, RAG retrieval, and a FastHTML UI.
The missing piece: **the agent can't see or control the IRIS production it
depends on.** When the ingestion pipeline stalls (as happened with the 24-hour
stall that motivated spec 016), no agent knows.

With `interop_*` tools wired into `iris-dev`, the kg-ticket-resolver agent can:
1. Check production health before starting work
2. Post a finding to the agent bus when a queue backs up
3. Let a second agent (or a human via Claude Desktop) restart a troubled component
4. Show the whole loop live on stage at AIML75

### The Three Modalities

This spec covers all three ways an agent can interact with IRIS Interop:

**A. External agent → Production** (this spec, Path 1)  
Claude Desktop, iris-dev, any MCP client can build and monitor productions.
Uses Atelier REST API. No IRIS build requirement.

**B. Production Business Process gets an LLM brain** (this spec, Path 2)  
A `pyprod` Business Operation or ObjectScript Business Process delegates to
`%AI.Agent` via the `%AI.Interop.ToolSet` pattern. The production *is* the agent.
Requires new IRIS build with `%AI.*`.

**C. External agent building productions via natural language**  
"Create a HL7 routing production that listens on TCP 6661 and routes ADT^A08
to the patient index." Agent uses `interop_create_production`,
`interop_add_component`, `interop_set_setting` to build it step by step.
Works on Path 1.

---

## Tool Inventory

### Core Production Lifecycle (6 tools)

| Tool | Description | IRIS call |
|---|---|---|
| `interop_production_status` | Current state + per-item breakdown | `Ens.Director.GetProductionStatus` |
| `interop_production_start` | Start a named production | `Ens.Director.StartProduction` |
| `interop_production_stop` | Stop with timeout + force flag | `Ens.Director.StopProduction` |
| `interop_production_update` | Hot-apply config changes | `Ens.Director.UpdateProduction` |
| `interop_production_needs_update` | Check config drift | `Ens.Director.ProductionNeedsUpdate` |
| `interop_production_recover` | Recover troubled production | `Ens.Director.RecoverProduction` |

### Component Management (4 tools)

| Tool | Description | IRIS call |
|---|---|---|
| `interop_create_production` | Create a new production class | `Ens.Config.Production + SaveToClass + Compile` |
| `interop_add_component` | Add business host to production | `Ens.Config.Item + production.SaveToClass` |
| `interop_remove_component` | Remove a business host | `FindItemByConfigName + RemoveItem + SaveToClass` |
| `interop_list_component_classes` | List valid subclasses of a superclass | `%Dictionary.ClassDefinition` SQL query |

### Configuration (2 tools)

| Tool | Description | IRIS call |
|---|---|---|
| `interop_get_setting` | Read a business host setting | `Ens.Config.Setting` lookup |
| `interop_set_setting` | Create or update a setting | `Ens.Config.Setting` upsert |

### Observability (3 tools)

| Tool | Description | IRIS call |
|---|---|---|
| `interop_logs` | Recent log entries filtered by type/component | `Ens_Util.Log` SQL |
| `interop_queues` | All current message queues + depths | `Ens.Queue_Enumerate()` |
| `interop_message_search` | Search message archive by source/target/class | `Ens.MessageHeader` SQL |

**Total: 15 tools** (ported and extended from team-23's 14 tools)

---

## Path 1: Atelier REST Implementation

### Connection

All interop tools use the existing `IrisConnection` struct in
`src/iris/connection.rs`. No new connection type needed. Calls go to:
- Class method execution: `POST /api/atelier/v1/{ns}/action/query` for SQL
- Direct class method: `POST /api/atelier/v1/{ns}/action/class` for method calls

### Rust Tool Registration Pattern

Follow the existing pattern in `src/tools/mod.rs`:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InteropProductionStatusParams {
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub full_status: bool,
}

#[tool(
    name = "interop_production_status",
    description = "Returns the current state of the running IRIS Interoperability production. \
                   With full_status=true, includes per-component enabled state and live status."
)]
async fn interop_production_status(
    &self,
    Parameters(params): Parameters<InteropProductionStatusParams>,
    _ctx: RequestContext<RoleServer>,
) -> Result<CallToolResult, McpError> {
    // Call Ens.Director.GetProductionStatus via Atelier REST
    // ...
}
```

New file: `src/tools/interop.rs` — all 15 tools in one module.
Register in `src/tools/mod.rs` via `tool_router!` macro alongside existing tools.

### Reference Implementation

Team-23's `~/ws/team-23/src/mcp_server_iris/interoperability.py` is the
canonical Python reference. Port each tool function to Rust, replacing:
- `iris.classMethodString(...)` → Atelier REST `action/class` call
- `iris.classMethodValue(...)` → same
- SQL queries → Atelier REST `action/query` call
- `IRISReference` → not needed; Atelier REST returns JSON directly

---

## Path 2: %AI.ToolSet (ObjectScript)

### New Class: `AI.Interop.ToolSet`

```objectscript
Class AI.Interop.ToolSet Extends %AI.ToolSet
{

/// XData block registers all 15 tools with the ToolManager
XData Definition [ MimeType = application/json ]
{
  {
    "name": "IRIS Interoperability Tools",
    "description": "Manage and monitor IRIS Interoperability productions, components, and message flows.",
    "tools": [
      {
        "name": "interop_production_status",
        "description": "Returns the current state of the running production...",
        "parameters": {
          "namespace": {"type": "string", "default": "USER"},
          "full_status": {"type": "boolean", "default": false}
        }
      },
      ...
    ]
  }
}

Method interop_production_status(namespace As %String = "USER", full_status As %Boolean = 0) As %DynamicObject
{
    Set tSC = ##class(Ens.Director).GetProductionStatus(.tProdName, .tState)
    $$$ThrowOnError(tSC)
    Set result = ##class(%DynamicObject).%New()
    Set result.production = tProdName
    Set result.state = $Case(tState, 1:"Running", 2:"Stopped", 3:"Suspended", 4:"Troubled", :"Unknown")
    If full_status {
        // Add per-item breakdown
    }
    Return result
}

// ... remaining 14 tool methods

}
```

### iris-mcp-server wgproto path

When `iris-mcp-server` is running (new IRIS build with `%AI.*`):
1. Tool discovery via RAG finds `AI.Interop.ToolSet` in the ToolManager registry
2. Client calls `interop_production_status` → `iris-mcp-server` → wgproto → `%AI.MCP.Service` → `AI.Interop.ToolSet.interop_production_status()`
3. `%AI.Policy` layer applies (rate limiting, audit, RBAC) before dispatch

### pyprod Business Operation with %AI brain (Modality B)

A `pyprod` Business Operation that delegates to `%AI.Agent` using these tools:

```python
from intersystems_pyprod import BusinessOperation

class InteropMonitorBO(BusinessOperation):
    def OnMessage(self, request):
        # %AI.Agent uses AI.Interop.ToolSet tools to respond
        agent = iris.cls('%AI.Agent')
        session = agent.CreateSession(self.GetConfig('AgentConfig'))
        response = agent.Chat(session, request.AlertText)
        return Status.OK(), response
```

This is the "Production gets a brain" pattern from plaza-aicore — the same
`%AI.*` integration Tim Leavitt's group built, applied to Interop.

---

## Demo Scenario: kg-ticket-resolver + Agent Bus

**Goal**: Show live at AIML75 that agents coordinate through IRIS, not just
alongside it.

### The Script

1. `iris-dev mcp` running — Claude Desktop has all 23 + 15 = 38 tools
2. Open kg-ticket-resolver — it's processing iService tickets
3. Deliberately stall the ingestion production (stop a Business Service)
4. **Agent 1** (monitoring agent in Claude Desktop):
   - `interop_production_status` → "Troubled: TicketIngestionService stopped"
   - Posts finding to agent bus: `{ "title": "Ingestion stalled", "severity": "warning" }`
5. **Agent 2** (iris-ai session — this session):
   - Receives finding on startup via spec 016 situational awareness
   - Surfaces: "⚠️ Ingestion stalled — TicketIngestionService stopped 4 min ago"
6. User asks: "Fix it"
7. Agent 2: `interop_production_start("TicketIngestion")` → production resumes
8. Both agents see the resolution reflected in the bus

**The one-liner for the slide**: *"Two agents. One production. One knowledge graph. Zero coordination code."*

---

## kg-ticket-resolver Revival Checklist

Before AIML75, get kg-ticket-resolver working end-to-end as the demo substrate:

- [ ] Verify IRIS production is defined and can start/stop (`start_production.py` currently starts FastHTML, not an IRIS production — this needs fixing or a new `start_iris_production.py`)
- [ ] Confirm iService/Solr connection is live (support-tools MCP server is running)
- [ ] Wire `interop_*` tools into the agent loop — replace ad-hoc IRIS calls
- [ ] Add agent bus `post_finding` call to the monitoring agent when queues back up
- [ ] Run full loop end-to-end on `dpgenai1` or `los-iris`

---

## Implementation Sequencing

### Phase 1 — Path 1 (Atelier REST) in Rust (~3-4 days)
1. Create `src/tools/interop.rs` with all 15 tool structs + stubs
2. Port 6 lifecycle tools from team-23 Python → Atelier REST calls
3. Port 4 component management tools
4. Port 2 configuration tools
5. Port 3 observability tools
6. Register in `tool_router!`, update tool count in README (23 → 38)
7. Integration test against local IRIS

### Phase 2 — Path 2 (%AI.ToolSet) in ObjectScript (~2 days)
1. Write `AI.Interop.ToolSet` with XData Definition block
2. Implement all 15 methods (can reuse SQL from Phase 1 testing)
3. Compile + test against new IRIS build (IRIS-2026.2.0AI.124.0)
4. Verify `iris-mcp-server` discovers the toolset via RAG

### Phase 3 — Demo wiring (~1 day)
1. Update kg-ticket-resolver to use `interop_*` tools
2. Add `post_finding` calls to agent bus
3. Full rehearsal on `dpgenai1`

### Phase 4 — pyprod Modality B (~1 day, stretch)
1. Write `InteropMonitorBO` pyprod class
2. Wire to `%AI.Agent` with `AI.Interop.ToolSet`
3. Add to demo as "the production monitors itself" beat

---

## Open Questions

1. **Atelier REST method calls**: Does `%api.atelier` support direct class method invocation, or do we need to use SQL + stored procedures for everything? Team-23 uses Native API `classMethodString` — we need to find the Atelier REST equivalent or write thin ObjectScript wrapper methods.

2. **Namespace handling**: The existing iris-dev tools default to `USER`. Interop productions typically live in `IRISHEALTH`, `HEALTHSHARE`, or a custom namespace. Should `interop_*` tools use a separate `--interop-namespace` flag, or follow the same discovery as other tools?

3. **`%AI.Policy` for interop tools**: `interop_production_stop` and `interop_production_start` are destructive operations. On Path 2, should these require an explicit policy grant? What's the default?

4. **kg-ticket-resolver production class**: Does a formal IRIS production class exist in kg-ticket-resolver, or is the "production" purely conceptual (Python processes)? If it's Python-only, the demo scenario needs a real ObjectScript production to demonstrate the lifecycle tools.
