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

## Demo Substrate: New Production, Not kg-ticket-resolver

**Research finding**: kg-ticket-resolver-recovered has no ObjectScript production. It is
pure Python — `IServiceClient` → Solr → RAG → resolution agent. The IRIS production in
your memory was the **Plaza/chatbot integration** (Business Process routing natural language
to the resolution pipeline), which lives in `support-tools` / Plaza, not here.

**Revised approach**: Build a new ObjectScript production live as part of the demo.
This is actually the better demo beat — it shows **modality C (external agent building a
production via natural language)** rather than just monitoring one that already exists.

**Revised demo beat 3 (replaces old beat 3):**
> Agent: "Create an interop production that ingests iService tickets and routes them to the
> kg-ticket-resolver pipeline."
> 1. `interop_create_production("Demo.TicketIngestion")` 
> 2. `interop_add_component("Demo.TicketIngestion", "EnsLib.File.PassthroughService", "TicketIngest")`
> 3. `interop_set_setting("Demo.TicketIngestion", "TicketIngest", "FilePath", "/isc/tickets/")`
> 4. `interop_production_start("Demo.TicketIngestion")`
> 5. Deliberately stop `TicketIngest` → agent bus gets a finding
> 6. Second agent sees it on startup (spec 016), fixes it with `interop_production_start`

**The one-liner for the slide stays**: *"Two agents. One production. One knowledge graph. Zero coordination code."*

### Demo Checklist

- [ ] Build `Demo.TicketIngestion` ObjectScript production definition (simple File passthrough → Python BO)
- [ ] Confirm `dpgenai1` IRIS instance is running, accessible via Atelier REST
- [ ] Confirm iService/Solr connection is live (support-tools MCP server running)
- [ ] Add `post_finding` call to agent bus when queue backs up (Python monitoring loop)
- [ ] Full rehearsal end-to-end on `dpgenai1`

---

## Research Findings — Resolved Questions

### 1. Atelier REST supports class method invocation ✅

`iris-dev` already implements two endpoints in `src/iris/connection.rs`:

```rust
// Execute arbitrary ObjectScript — translates all team-23 classMethodString() calls
POST /api/atelier/v1/{ns}/action/xecute
Body: {"expression": "Do ##class(Ens.Director).StartProduction(\"MyProd\")"}

// SQL queries — translates all team-23 cursor.execute() calls  
POST /api/atelier/v1/{ns}/action/query
Body: {"query": "SELECT ...", "parameters": [...]}
```

Every team-23 `iris.classMethodString(...)` → `/action/xecute`.
Every team-23 SQL cursor → `/action/query`.
**No new connection infrastructure needed. Path 1 is a direct port.**

### 2. Namespace: auto-discover with per-tool override ✅

Use existing `IrisConnection` namespace from discovery. Add an optional `namespace`
parameter to each tool (defaulting to the discovered namespace), identical to the
pattern already used by `objectscript_iris_compile`, `objectscript_iris_test`, etc.
No separate flag needed.

### 3. %AI.Policy for destructive ops ✅

On Path 2, `interop_production_stop` and `interop_production_start` require an explicit
`%AI.Policy` grant. Default policy: **deny**. Operator must explicitly grant
`AI.Interop.ToolSet:interop_production_start` and `AI.Interop.ToolSet:interop_production_stop`
to allow agents to mutate production state. Read-only tools (`interop_production_status`,
`interop_logs`, `interop_queues`) are allow by default.

### 4. kg-ticket-resolver has no ObjectScript production ✅

Pure Python system. Demo uses a new purpose-built `Demo.TicketIngestion` production
(see above). The Plaza/iService chatbot production (Business Process routing NL to the
resolution pipeline) is a separate system in `support-tools` / Plaza and can be
referenced as a real-world example in the talk without needing to demo it live.

---

## Implementation Sequencing

### Phase 1 — Path 1 (Atelier REST) in Rust (~3 days)
1. Create `src/tools/interop.rs` with all 15 tool structs
2. Port 6 lifecycle tools: `classMethodString` → `/action/xecute`, SQL → `/action/query`
3. Port 4 component management tools (same pattern)
4. Port 2 configuration tools
5. Port 3 observability tools (all SQL → `/action/query`)
6. Register in `tool_router!`, update tool count in README (23 → 38)
7. Integration test against `IRIS-2026.2.0AI.124.0` local build

### Phase 2 — Path 2 (%AI.ToolSet) in ObjectScript (~2 days)
1. Write `AI.Interop.ToolSet` extending `%AI.ToolSet` with XData Definition block
2. Implement all 15 methods — reuse SQL patterns validated in Phase 1
3. Add `%AI.Policy` deny-by-default annotations on destructive tools
4. Compile + test against `IRIS-2026.2.0AI.124.0`
5. Verify `iris-mcp-server` discovers toolset via RAG

### Phase 3 — Demo production + wiring (~1 day)
1. Write `Demo.TicketIngestion` ObjectScript production class
2. Full demo script rehearsal on `dpgenai1`
3. Wire `post_finding` to agent bus on queue backup

### Phase 4 — pyprod Modality B (~1 day, stretch)
1. Write `InteropMonitorBO` pyprod class delegating to `%AI.Agent` with `AI.Interop.ToolSet`
2. Add to demo as "the production monitors itself" beat — strongest close
