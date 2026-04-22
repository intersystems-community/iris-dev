# Data Model: iris-dev v2

## Core Entities

### IrisConnection
```rust
pub struct IrisConnection {
    pub base_url: String,         // http://host:port[/prefix] — prefix already included
    pub namespace: String,
    pub username: String,
    pub password: String,
    pub atelier_version: AtelierVersion,  // NEW: V8 | V2 | V1
    pub source: DiscoverySource,
    pub port_superserver: Option<u16>,    // retained for optional native use
    pub version: Option<String>,          // IRIS version string
}

pub enum AtelierVersion { V8, V2, V1 }

pub enum DiscoverySource {
    ExplicitFlag,
    EnvVar,
    LocalhostScan { port: u16 },
    Docker { container_name: String },
    VsCodeSettings,
}
```

### ToolError (unified error envelope)
```rust
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    pub iris_error: Option<IrisError>,
    pub details: Option<serde_json::Value>,
}

pub struct IrisError {
    pub code: i32,
    pub domain: String,
    pub id: String,
    pub params: Vec<String>,
}

// Serialises to: {"success":false,"error_code":"...","error":"...","iris_error":{...}}
```

### Skill
```rust
// Stored in ^SKILLS(name) as $lb(description, body, usage_count, created_at)
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,              // reusable prompt or pattern
    pub usage_count: u32,
    pub created_at: String,        // ISO8601
}
```

### SessionHistory (in-memory ring buffer)
```rust
pub struct ToolCall {
    pub tool: String,
    pub input: serde_json::Value,
    pub success: bool,
    pub timestamp: std::time::Instant,
}
// Ring buffer: VecDeque<ToolCall>, capacity 50
```

### SearchResult
```rust
pub struct SearchResult {
    pub document: String,
    pub line: u32,
    pub member: Option<String>,    // method/property name if applicable
    pub content: String,           // matched line text
}
// Truncated at 200 results with truncated:true flag
```

---

## Tool Parameter Schemas

### iris_doc
```rust
pub struct IrisDocParams {
    pub mode: DocMode,             // required: get|put|delete|head
    pub name: Option<String>,      // required for single-doc ops
    pub names: Option<Vec<String>>,// for batch ops (mode=get with multiple, mode=delete with multiple)
    pub content: Option<String>,   // required for mode=put
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
pub enum DocMode { Get, Put, Delete, Head }
```

### iris_compile
```rust
pub struct CompileParams {
    pub target: String,            // "MyApp.Patient.cls" or "MyApp.*.cls"
    #[serde(default = "default_flags")]  // "cuk"
    pub flags: String,
    #[serde(default)]
    pub force_writable: bool,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
```

### iris_execute
```rust
pub struct ExecuteParams {
    pub code: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default = "default_execute_timeout")]  // 30
    pub timeout_secs: u32,
}
```

### iris_query
```rust
pub struct QueryParams {
    pub query: String,
    #[serde(default)]
    pub parameters: Vec<serde_json::Value>,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
```

### iris_search
```rust
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    pub category: Option<String>,       // "CLS"|"MAC"|"INT"|"INC"|"ALL"
    #[serde(default)]
    pub documents: Vec<String>,         // wildcard scopes e.g. ["HS.FHIR.*.cls"]
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
```

### iris_macro
```rust
pub struct MacroParams {
    pub action: MacroAction,        // required
    pub name: Option<String>,       // required except for action=list
    #[serde(default)]
    pub args: Vec<String>,          // for action=expand
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
pub enum MacroAction { List, Signature, Location, Definition, Expand }
```

### interop_production
```rust
pub struct InteropProductionParams {
    pub action: ProductionAction,   // required
    pub production: Option<String>, // required for action=start
    #[serde(default = "default_timeout_30")]
    pub timeout: u32,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}
pub enum ProductionAction { Status, Start, Stop, Update, NeedsUpdate, Recover }
```

### skill
```rust
pub struct SkillParams {
    pub action: SkillAction,        // required
    pub name: Option<String>,       // for describe/forget
    pub query: Option<String>,      // for search
}
pub enum SkillAction { List, Describe, Search, Forget, Propose }
```

---

## State Transitions

### iris_put_doc with IRIS_SOURCE_CONTROL=true
```
call OnBeforeSave(name) via xecute
    ↓ error → return SCM_REJECTED error, abort PUT
    ↓ ok
PUT /api/atelier/v8/{ns}/doc/{name}
    ↓ 409 → HEAD to get ETag, retry PUT with If-None-Match
    ↓ 409 again → return CONFLICT error
    ↓ 2xx
call OnAfterSave(name) via xecute (best-effort, don't fail on error)
    ↓
return success
```

### iris_search async fallback
```
POST /api/atelier/v2/{ns}/action/search (sync)
    ↓ responds within 2s → return results
    ↓ timeout
POST /api/atelier/v2/{ns}/action/search (async) → { workId: "abc" }
    ↓ poll GET /action/search?workId=abc every 2s, up to 5 minutes
    ↓ result ready → return results
    ↓ 5-minute timeout → return SEARCH_TIMEOUT error
```

### Discovery cascade
```
Explicit conn provided? → use it
    ↓ no
IRIS_HOST env set? → probe (5s timeout)
    ↓ no / unreachable
Parallel scan [52773, 41773, 51773, 8080] (100ms each)
    ↓ none respond
Docker bollard scan (score by workspace basename)
    ↓ no containers
VS Code settings.json (objectscript.conn + intersystems.servers)
    ↓ not found
conn = None → tools return IRIS_UNREACHABLE
```
