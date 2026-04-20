# Research: iris-dev v2.1 — SCM Elicitation, Auto-Open, SCM Tools

**Date**: 2026-04-20
**Branch**: 020-scm-elicitation-auto-open

---

## Decision 1: MCP Elicitation Message Format

**Decision**: Use the official MCP `elicitation/create` JSON-RPC method as the primary path. Fall back to structured JSON in the tool result when the client does not advertise `"elicitation": {}` capability.

**Primary path — `elicitation/create` request** (server → client):
```json
{
  "jsonrpc": "2.0",
  "id": "elicit-1",
  "method": "elicitation/create",
  "params": {
    "message": "MyApp.Patient.cls is checked out to Nathan Keast. Check out and overwrite?",
    "requestedSchema": {
      "type": "object",
      "properties": {
        "confirm": {
          "type": "boolean",
          "description": "Yes to check out and write, No to abort"
        }
      },
      "required": ["confirm"]
    }
  }
}
```

**Client response** (client → server):
```json
{"jsonrpc":"2.0","id":"elicit-1","result":{"action":"accept","content":{"confirm":true}}}
// or
{"jsonrpc":"2.0","id":"elicit-1","result":{"action":"decline"}}
// or
{"jsonrpc":"2.0","id":"elicit-1","result":{"action":"cancel"}}
```

**Client capability check** — during `initialize`, if `clientInfo.capabilities.elicitation` is present, use the formal path. Otherwise fall back.

**Fallback path** (structured JSON in tool result):
```json
{
  "success": false,
  "elicitation_required": true,
  "elicitation_id": "elicit-abc123",
  "message": "MyApp.Patient.cls is checked out to Nathan Keast. Check out and overwrite?",
  "options": ["yes", "no"]
}
```
The AI client presents this as a question; the user's answer is passed back via `iris_doc(mode=put, elicitation_answer="yes", elicitation_id="elicit-abc123")`.

**rmcp support**: rmcp 1.2 (current) does NOT have first-class Elicitation. Must send `elicitation/create` via raw JSON-RPC using rmcp's escape hatch for non-standard messages. Track rmcp upstream for native support.

---

## Decision 2: IRIS Source Control Hook Patterns

**Base class**: `%Studio.SourceControl.Base`

**OnBeforeSave pattern via Atelier xecute**:
```objectscript
set sc=##class(%Studio.SourceControl.ISC).OnBeforeSave("MyApp.Patient.cls")
if $system.Status.IsError(sc) {
  set msg=$system.Status.GetErrorText(sc)
  write "ERROR:"_msg
} else {
  write "OK"
}
```

**UserAction response codes** (Action output param from UserAction method):
| Code | Meaning | What to do |
|------|---------|-----------|
| 0 | No action | Proceed |
| 1 | Show confirmation dialog | → Elicitation (Yes/No) |
| 2 | Show popup URL | → Return URL as hint, not supported interactively |
| 3 | Execute external program | → Return error (out of scope) |
| 4 | Insert text into editor | → Return text in response |
| 5 | Open file | → Populate open_uri sentinel |
| 6 | Show alert | → Return as error message |
| 7 | Show text prompt dialog | → Elicitation (free text input) |

**GetMenu/OnMenuItem pattern** — retrieve available SCM actions for a document:
```objectscript
set sc=##class(%Studio.SourceControl.Base).OnMenuItem(
  "%SourceMenu,Status","MyApp.Patient.cls","",
  .enabled,.displayName)
write enabled_"|"_displayName
```

To get the full menu, iterate known menu item names from XData — or call `GetMenu` if available in the installed SCM class.

**Check out pattern** (most common SCM action):
```objectscript
set action=""
set target=""
set msg=""
set reload=0
set sc=##class(%Studio.SourceControl.ISC).UserAction(
  0,"%SourceMenu,CheckOut","MyApp.Patient.cls","",
  .action,.target,.msg,.reload)
// action=0 means checkout happened, action=1 means dialog needed
write action_"|"_msg
```

---

## Decision 3: Sentinel File for Auto-Open

**Decision**: Binary writes `~/.iris-dev/open-hint.json` after successful `iris_doc(mode=put)` or `iris_compile`. Extension uses `vscode.workspace.createFileSystemWatcher` to watch this file.

**Sentinel file format**:
```json
{"uri": "isfs://USER/MyApp.Patient.cls", "ts": 1745159847000}
```

**Extension watcher logic** (TypeScript):
```typescript
const watcher = vscode.workspace.createFileSystemWatcher(
  new vscode.RelativePattern(os.homedir(), '.iris-dev/open-hint.json')
);
watcher.onDidChange(async () => {
  const hint = JSON.parse(await fs.readFile(hintPath, 'utf8'));
  if (Date.now() - hint.ts < 3000 && hasIsfsWorkspace()) {
    await vscode.window.showTextDocument(vscode.Uri.parse(hint.uri));
  }
});
```

**Sentinel file location**: `~/.iris-dev/open-hint.json` (cross-platform: `%USERPROFILE%\.iris-dev\open-hint.json` on Windows).

---

## Decision 4: Elicitation State Management

**Decision**: In-memory `HashMap<String, PendingElicitation>` keyed by `elicitation_id` (UUID). Expiry checked lazily on access. No persistence — state lost on server restart (acceptable; MCP sessions don't survive restarts).

**PendingElicitation fields**:
```rust
struct PendingElicitation {
    id: String,           // UUID
    document: String,     // e.g. "MyApp.Patient.cls"
    action: String,       // "put" | "scm_execute"
    scm_action_id: Option<String>,
    content: Option<String>,  // document content for put resumption
    namespace: String,
    expires_at: Instant,  // now + 5 minutes
}
```

---

## Decision 5: iris_source_control Tool — OnMenu Approach

Rather than hardcoding menu item names, call `UserAction` with `Type=0` and known menu names to discover what's available. The installed SCM class's `OnMenuItem` method returns `enabled=0` for unavailable items — use this to build the menu dynamically.

**Practical approach**: Call `OnMenuItem` for a standard set of known menu item names (`CheckOut`, `UndoCheckOut`, `CheckIn`, `GetLatest`, `Status`, `History`, `AddToSourceControl`). Return only those where `enabled=1` as the available menu.

**iris_source_control(action=status)** pattern:
```objectscript
set obj=##class(%Studio.SourceControl.Base).%GetImplementationObject("MyApp.Patient.cls")
if '$IsObject(obj) { write "UNCONTROLLED" quit }
set isEditable=obj.IsEditable("MyApp.Patient.cls")
write isEditable_"|"_$get(obj.Owner)
```
