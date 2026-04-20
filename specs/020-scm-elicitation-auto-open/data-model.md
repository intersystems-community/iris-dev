# Data Model: iris-dev v2.1 — SCM Elicitation

## PendingElicitation (in-memory, session-scoped)
```rust
struct PendingElicitation {
    id: String,                    // UUID, used as elicitation_id
    document: String,              // "MyApp.Patient.cls"
    action: ElicitationAction,     // Put | ScmExecute
    content: Option<String>,       // document content (for Put resumption)
    scm_action_id: Option<String>, // SCM menu item ID (for ScmExecute resumption)
    namespace: String,
    expires_at: std::time::Instant,
}

enum ElicitationAction { Put, ScmExecute }
// Stored as: Arc<Mutex<HashMap<String, PendingElicitation>>>
// Added to IrisTools struct alongside client, history
```

## OpenHint (sentinel file)
```json
// ~/.iris-dev/open-hint.json
{"uri": "isfs://USER/MyApp.Patient.cls", "ts": 1745159847000}
```
Written by: `iris_doc(mode=put)`, `iris_compile` (single document)
Read by: vscode-iris-dev extension FileSystemWatcher

## IrisDocParams (extended)
```rust
struct IrisDocParams {
    mode: DocMode,
    name: Option<String>,
    names: Option<Vec<String>>,
    content: Option<String>,
    namespace: String,
    // NEW:
    elicitation_answer: Option<String>,  // "yes"/"no"/free text from prior elicitation
    elicitation_id: Option<String>,      // UUID from prior elicitation response
}
```

## ScmStatus
```rust
struct ScmStatus {
    controlled: bool,
    editable: bool,
    locked: bool,
    owner: Option<String>,  // username of lock holder
}
// Serialises to: {"controlled":true,"editable":false,"locked":true,"owner":"nkeast"}
```

## ScmAction
```rust
struct ScmAction {
    id: String,       // e.g. "CheckOut"
    label: String,    // e.g. "Check Out"
    enabled: bool,
}
// Returned as array in iris_source_control(action=menu) response
```

## State Transitions: iris_doc(mode=put) with SCM

```
iris_doc(mode=put, name, content)
    ↓
OnBeforeSave() via xecute
    ↓ OK (action=0)              → write document → write sentinel → return success
    ↓ dialog needed (action=1)   → create PendingElicitation → send elicitation/create
                                   (or structured JSON fallback)
    ↓ error                      → return SCM_REJECTED error

elicitation response: action=accept, confirm=true
    ↓
look up PendingElicitation by id
    ↓
checkout via UserAction(CheckOut) → write document → write sentinel → return success

elicitation response: action=decline or cancel
    ↓
clear PendingElicitation → return WRITE_ABORTED
```

## State Transitions: iris_source_control(action=execute)
```
execute(document, action_id)
    ↓
UserAction(Type=0, Name=action_id, document)
    ↓ action=0   → done, return success
    ↓ action=1   → elicitation required → create PendingElicitation(ScmExecute)
    ↓ action=7   → free text prompt → elicitation with string field

elicitation response received
    ↓
AfterUserAction(document, action_id, answer) → return result
```
