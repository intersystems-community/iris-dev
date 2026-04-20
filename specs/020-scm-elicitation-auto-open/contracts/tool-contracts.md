# Tool Contracts: iris-dev v2.1 — SCM Tools

---

## iris_doc(mode=put) — extended

**Normal success** (no SCM or SCM allows immediately):
```json
{"success": true, "name": "MyApp.Patient.cls", "open_uri": "isfs://USER/MyApp.Patient.cls"}
```

**Elicitation required — formal path** (client supports elicitation):
Server sends `elicitation/create` out-of-band. Tool call is suspended until response.

**Elicitation required — fallback** (client does not support elicitation):
```json
{
  "success": false,
  "elicitation_required": true,
  "elicitation_id": "a1b2c3d4-...",
  "message": "MyApp.Patient.cls is checked out to Nathan Keast. Check out and overwrite?",
  "options": ["yes", "no"]
}
```

**Resume after elicitation** (user answered yes):
```json
// Input:
{"mode": "put", "name": "MyApp.Patient.cls", "content": "...",
 "elicitation_id": "a1b2c3d4-...", "elicitation_answer": "yes"}
// Output:
{"success": true, "name": "MyApp.Patient.cls", "open_uri": "isfs://USER/MyApp.Patient.cls"}
```

**SCM rejected**:
```json
{"success": false, "error_code": "SCM_REJECTED", "error": "File is locked: checkout denied"}
```

**Write aborted by user**:
```json
{"success": false, "error_code": "WRITE_ABORTED", "error": "User declined checkout"}
```

**Elicitation timed out**:
```json
{"success": false, "error_code": "SCM_TIMEOUT", "error": "Elicitation expired after 5 minutes"}
```

---

## iris_source_control

### action=status
```json
// Input:
{"action": "status", "document": "MyApp.Patient.cls", "namespace": "USER"}
// Output (locked):
{"success": true, "controlled": true, "editable": false, "locked": true, "owner": "nkeast"}
// Output (editable):
{"success": true, "controlled": true, "editable": true, "locked": false, "owner": null}
// Output (uncontrolled):
{"success": true, "controlled": false, "editable": true, "locked": false, "owner": null}
```

### action=menu
```json
// Input:
{"action": "menu", "document": "MyApp.Patient.cls", "namespace": "USER"}
// Output:
{"success": true, "document": "MyApp.Patient.cls", "actions": [
  {"id": "CheckOut",        "label": "Check Out",         "enabled": true},
  {"id": "UndoCheckOut",    "label": "Undo Check Out",    "enabled": false},
  {"id": "CheckIn",         "label": "Check In",          "enabled": false},
  {"id": "GetLatest",       "label": "Get Latest Version","enabled": true},
  {"id": "History",         "label": "View History",      "enabled": true}
]}
```

### action=checkout
```json
// Input:
{"action": "checkout", "document": "MyApp.Patient.cls", "namespace": "USER"}
// Output (success):
{"success": true, "document": "MyApp.Patient.cls", "editable": true}
// Output (needs elicitation — force checkout from another user):
{"success": false, "elicitation_required": true, "elicitation_id": "...",
 "message": "Force check out from Nathan Keast?", "options": ["yes", "no"]}
```

### action=execute
```json
// Input:
{"action": "execute", "document": "MyApp.Patient.cls", "action_id": "CheckIn",
 "namespace": "USER"}
// Output (requires comment — text prompt elicitation):
{"success": false, "elicitation_required": true, "elicitation_id": "...",
 "message": "Check-in comment:", "input_type": "text"}
// Resume:
{"action": "execute", "document": "MyApp.Patient.cls", "action_id": "CheckIn",
 "elicitation_id": "...", "answer": "Fixed bug #123"}
// Final output:
{"success": true, "document": "MyApp.Patient.cls", "action_id": "CheckIn"}
```

---

## iris_compile — extended (open_uri added)

```json
// Input: (unchanged)
{"target": "MyApp.Patient.cls", "flags": "cuk", "namespace": "USER"}
// Output (success, single document):
{"success": true, "target": "MyApp.Patient.cls", "errors": [], "warnings": [],
 "open_uri": "isfs://USER/MyApp.Patient.cls"}
// Output (wildcard / multiple — no open_uri):
{"success": true, "targets_compiled": 12, "errors": [], "warnings": []}
```
