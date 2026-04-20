# Tool Contracts: iris-dev v2

All tools return newline-delimited JSON over stdio (MCP protocol). Every response content is a JSON string with the envelope:

**Success**: `{"success": true, ...tool-specific fields}`
**Failure**: `{"success": false, "error_code": "CODE", "error": "message", "iris_error": {...}?}`

---

## iris_compile

**Input**:
```json
{"target": "MyApp.Patient.cls", "flags": "cuk", "force_writable": false, "namespace": "USER"}
```
**Output (success)**:
```json
{"success": true, "target": "MyApp.Patient.cls", "errors": [], "warnings": [], "console": []}
```
**Output (compile errors)**:
```json
{"success": false, "target": "MyApp.Patient.cls", "errors": [
  {"line": 42, "column": 5, "code": "W1", "severity": "warning", "text": "Method never called"}
]}
```
**Error codes**: `IRIS_UNREACHABLE`, `NOT_FOUND`, `COMPILE_ERROR`

---

## iris_execute

**Input**:
```json
{"code": "write $ZVERSION,!", "namespace": "USER", "timeout_secs": 30}
```
**Output**:
```json
{"success": true, "output": "IRIS for UNIX (Apple M1) 2024.1..."}
```
**Error codes**: `IRIS_UNREACHABLE`, `IRIS_ERROR`, `TIMEOUT`
**IRIS_ERROR includes**: `iris_error: {"code": -1, "domain": "ObjectScript", "id": "UNDEFINED", "params": ["x"]}`

---

## iris_query

**Input**:
```json
{"query": "SELECT TOP 5 Name FROM %Dictionary.ClassDefinition", "parameters": [], "namespace": "USER"}
```
**Output**:
```json
{"success": true, "rows": [{"Name": "MyApp.Order"}, {"Name": "MyApp.Patient"}], "count": 2}
```
**Error codes**: `IRIS_UNREACHABLE`, `SQL_ERROR`

---

## iris_test

**Input**:
```json
{"pattern": "MyApp.Tests", "namespace": "USER"}
```
**Output**:
```json
{"success": true, "passed": 12, "failed": 0, "total": 12, "output": "...full trace..."}
```
**Output (failures)**:
```json
{"success": false, "passed": 10, "failed": 2, "total": 12, "output": "..."}
```
**Error codes**: `IRIS_UNREACHABLE`, `TEST_ERROR`

---

## iris_search

**Input**:
```json
{"query": "GetOrderStatus", "regex": false, "case_sensitive": false, "category": "CLS", "documents": [], "namespace": "USER"}
```
**Output**:
```json
{"success": true, "results": [
  {"document": "MyApp.Order.cls", "line": 42, "member": "GetOrderStatus", "content": "Method GetOrderStatus()..."}
], "truncated": false, "total_found": 3}
```
**Error codes**: `IRIS_UNREACHABLE`, `SEARCH_TIMEOUT`

---

## iris_doc

**Get** (`mode=get`):
```json
// Input: {"mode": "get", "name": "MyApp.Patient.cls", "namespace": "USER"}
// Output: {"success": true, "name": "MyApp.Patient.cls", "content": "Class MyApp.Patient...", "timestamp": "..."}
```

**Put** (`mode=put`):
```json
// Input: {"mode": "put", "name": "MyApp.Patient.cls", "content": "Class MyApp.Patient...", "namespace": "USER"}
// Output: {"success": true, "name": "MyApp.Patient.cls"}
// Error: {"success": false, "error_code": "CONFLICT", "error": "Document modified by another user"}
// Error: {"success": false, "error_code": "SCM_REJECTED", "error": "Source control rejected: ..."}
```

**Delete** (`mode=delete`):
```json
// Input: {"mode": "delete", "name": "MyApp.Patient.cls", "namespace": "USER"}
// Output: {"success": true, "name": "MyApp.Patient.cls"}
```

**Head** (`mode=head`):
```json
// Input: {"mode": "head", "name": "MyApp.Patient.cls", "namespace": "USER"}
// Output: {"success": true, "exists": true, "timestamp": "2026-04-19T10:00:00Z"}
// Output: {"success": true, "exists": false}
```

**Error codes**: `IRIS_UNREACHABLE`, `NOT_FOUND`, `CONFLICT`, `SCM_REJECTED`, `FORBIDDEN`

---

## iris_macro

**List** (`action=list`):
```json
// Input: {"action": "list", "namespace": "USER"}
// Output: {"success": true, "macros": ["$$$ISERR", "$$$ThrowOnError", ...]}
```

**Signature** (`action=signature`):
```json
// Input: {"action": "signature", "name": "$$$ThrowOnError", "namespace": "USER"}
// Output: {"success": true, "name": "$$$ThrowOnError", "signature": "(pStatus)"}
```

**Expand** (`action=expand`):
```json
// Input: {"action": "expand", "name": "$$$ThrowOnError", "args": ["sc"], "namespace": "USER"}
// Output: {"success": true, "expanded": "if $$$ISERR(sc) { throw ##class(%Exception.StatusException).CreateFromStatus(sc) }"}
```

---

## interop_production

**Status** (`action=status`):
```json
// Output: {"success": true, "production": "MyApp.MainProd", "state": "Running", "item_count": 15}
```

**Start** (`action=start`):
```json
// Input: {"action": "start", "production": "MyApp.MainProd", "namespace": "ENSEMBLE"}
// Output: {"success": true, "production": "MyApp.MainProd", "state": "Running"}
```

**Stop** (`action=stop`):
```json
// Input: {"action": "stop", "timeout": 30, "force": false, "namespace": "ENSEMBLE"}
// Output: {"success": true, "state": "Stopped"}
```

**Error codes**: `IRIS_UNREACHABLE`, `NOT_ENSEMBLE`, `PRODUCTION_ERROR`

---

## interop_query

**Logs** (`what=logs`):
```json
// Input: {"what": "logs", "item_name": "MyBP", "log_type": "error", "limit": 10, "namespace": "ENSEMBLE"}
// Output: {"success": true, "logs": [{"time": "...", "item": "MyBP", "type": "error", "text": "..."}]}
```

**Queues** (`what=queues`):
```json
// Output: {"success": true, "queues": [{"name": "Ens.Queue.Input", "count": 0}]}
```

**Messages** (`what=messages`):
```json
// Input: {"what": "messages", "source": "MyBS", "limit": 20, "namespace": "ENSEMBLE"}
// Output: {"success": true, "messages": [{"session_id": "...", "created": "...", "status": "Completed"}]}
```

---

## skill

**List** (`action=list`):
```json
// Output: {"success": true, "skills": [{"name": "compile-and-test", "description": "...", "usage_count": 5}]}
```

**Propose** (`action=propose`):
```json
// Output: {"success": true, "skill": {"name": "debug-undefined", "description": "Maps INT errors to source"}}
// Output (insufficient calls): {"success": false, "error_code": "INSUFFICIENT_HISTORY", "error": "Need at least 5 tool calls"}
```

---

## agent_info

**Input**: `{"what": "stats"}`
**Output**: `{"success": true, "skill_count": 12, "session_calls": 47, "learning_enabled": true}`

**Input**: `{"what": "history", "limit": 10}`
**Output**: `{"success": true, "calls": [{"tool": "iris_compile", "success": true, "ago_secs": 42}]}`
