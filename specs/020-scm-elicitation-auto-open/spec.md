# Feature Specification: iris-dev v2.1 — Source Control Elicitation, Auto-Open, and SCM Tools

**Feature Branch**: `020-scm-elicitation-auto-open`
**Created**: 2026-04-20
**Status**: Draft
**Contributors**: Tom Dyar, Tim Leavitt (isfs-mcp), Nathan Keast (intersystems-mcp-atelier)

## Overview

Three InterSystems developers independently built IRIS MCP servers and converged on the same pain points. This feature consolidates their solutions into iris-dev:

1. **Source control dialogs** — IRIS server-side source control (Studio hooks) generates popup dialogs that have no equivalent when an AI assistant is writing code. The fix is MCP Elicitation: pause the tool call, ask the question in the chat, resume when the user answers.

2. **Auto-open after write** — When an AI creates or modifies a class, the developer can't see it without manually navigating. The tool should signal the VS Code extension to open it automatically.

3. **SCM tools** — Developers need to inspect lock status, check out files, and execute source control actions directly from the chat, without knowing which SCM system is installed.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Write to a Source-Controlled Document (Priority: P1)

A developer asks Copilot to modify a class. The class is checked out to another user in the IRIS source control system. Instead of a popup dialog or silent failure, Copilot asks "This file is checked out to Nathan Keast. Check out anyway?" in the chat. The developer says yes, the file is checked out, and the write completes — all without leaving the chat.

**Why this priority**: This is the core blocker Nathan and Tim both identified. Every team using IRIS server-side source control hits this. Without it, AI-assisted editing is broken for source-controlled workspaces.

**Independent test**: With a source-controlled IRIS namespace, call `iris_doc(mode=put)` on a locked document. Verify an elicitation question appears in the chat, not a popup or error.

**Acceptance Scenarios**:

1. **Given** a document is checked out to another user, **When** `iris_doc(mode=put)` is called, **Then** an elicitation question appears asking whether to check out, with Yes/No options
2. **Given** the user answers Yes to the elicitation, **When** the tool resumes, **Then** the document is checked out and saved successfully
3. **Given** the user answers No to the elicitation, **When** the tool resumes, **Then** the write is aborted with a clear message and no side effects
4. **Given** the MCP client does not support elicitation, **When** `iris_doc(mode=put)` encounters a locked document, **Then** a descriptive error is returned explaining the lock and who holds it — no silent failure, no popup
5. **Given** no source control system is installed in the IRIS namespace, **When** `iris_doc(mode=put)` is called, **Then** the write proceeds immediately with no elicitation

---

### User Story 2 — Document Opens Automatically After Write (Priority: P1)

A developer asks Copilot to generate a new class. After the class is created and compiled, it opens automatically in VS Code's ISFS workspace — the developer doesn't need to navigate to it manually.

**Why this priority**: Completing the loop from "create" to "visible in editor" is essential UX. Without it, the developer has to hunt for the file they just asked the AI to create.

**Independent test**: With an ISFS workspace open in VS Code, ask Copilot to create and save a new class. Verify the file opens in the editor automatically.

**Acceptance Scenarios**:

1. **Given** an ISFS workspace folder is open in VS Code, **When** `iris_doc(mode=put)` succeeds, **Then** the document opens automatically in the VS Code editor
2. **Given** an ISFS workspace is open, **When** `iris_compile` succeeds on a single class, **Then** that class opens automatically in the editor
3. **Given** no ISFS workspace folder is open, **When** `iris_doc(mode=put)` succeeds, **Then** the open hint is silently ignored — no error, no broken behaviour
4. **Given** a batch operation writes multiple documents, **When** all succeed, **Then** only the last/primary document is auto-opened (not a flood of tabs)

---

### User Story 3 — Inspect and Execute Source Control Actions (Priority: P2)

A developer asks "what source control options are available for MyApp.Patient.cls?" and gets back a list of actions from the installed SCM system (e.g. "Check Out", "Undo Checkout", "Show History"). They can then execute any of those actions directly from the chat.

**Why this priority**: Different IRIS installations use different SCM systems (Perforce, Git, built-in). This tool works with whatever is installed by delegating to IRIS's own OnMenu hook.

**Independent test**: With a source-controlled namespace, call `iris_source_control(action=menu, document=MyApp.Patient.cls)`. Verify the response lists the same actions that appear in the Studio source control menu.

**Acceptance Scenarios**:

1. **Given** a SCM system is installed, **When** `iris_source_control(action=menu, document=MyApp.Patient.cls)` is called, **Then** available actions with IDs and labels are returned
2. **Given** the menu has been retrieved, **When** `iris_source_control(action=execute, document=..., action_id=...)` is called, **Then** the action runs and its result is returned
3. **Given** an action requires user input (e.g. a checkout comment), **When** the action is executed, **Then** an elicitation question is returned; when the user answers, the action completes
4. **Given** no SCM system is installed, **When** `iris_source_control(action=status)` is called, **Then** `{"controlled": false}` is returned — no error
5. **Given** `iris_source_control(action=checkout, document=...)` is called, **When** the document is not yet checked out, **Then** it is checked out and the new editable status is returned

---

### User Story 4 — Check Document Lock Status Before Editing (Priority: P2)

Before modifying a file, a developer (or the AI agent) can check whether it's editable, locked, or uncontrolled — and who holds the lock.

**Why this priority**: Proactive status check prevents the mid-write elicitation surprise. An AI agent can check status first and inform the user before attempting a write.

**Independent test**: Call `iris_source_control(action=status, document=MyApp.Patient.cls)` on a locked document. Verify the response includes `locked: true` and the lock owner's name.

**Acceptance Scenarios**:

1. **Given** a document is checked out by the current user, **When** `status` is called, **Then** `{"editable": true, "locked": false, "owner": "current_user"}` is returned
2. **Given** a document is checked out by another user, **When** `status` is called, **Then** `{"editable": false, "locked": true, "owner": "Nathan Keast"}` is returned
3. **Given** a document has never been added to source control, **When** `status` is called, **Then** `{"controlled": false, "editable": true}` is returned

---

### Edge Cases

- **Elicitation times out** (user doesn't respond within 5 minutes): pending state is cleared, tool returns `SCM_TIMEOUT` error
- **SCM hook OnBeforeSave raises an ObjectScript error**: return structured error with the ObjectScript error text, do not crash
- **ISFS workspace open but document URI can't be resolved**: log a warning, do not fail the write operation
- **Nested elicitation** (an action triggers another dialog): handle up to 3 levels of elicitation depth; beyond that, return an error
- **Auto-open for .mac or .int files**: open as plain text, not expecting ISFS scheme if namespace doesn't expose them

---

## Requirements *(mandatory)*

### Functional Requirements

**Elicitation**

- **FR-001**: When `iris_doc(mode=put)` triggers an IRIS UserAction dialog (code 1), the tool MUST send a formal MCP `elicitation/create` request (spec-compliant JSON-RPC) as the primary path; when the client does not advertise elicitation capability, fall back to a structured JSON response containing the question and options
- **FR-002**: `iris_doc(mode=put)` MUST accept an optional `elicitation_answer` parameter to resume a pending elicitation
- **FR-003**: Pending elicitation state MUST expire after 5 minutes of inactivity
- **FR-004**: When the MCP client does not support elicitation, `iris_doc(mode=put)` MUST return a descriptive error including the lock owner — no silent failure, no popup
- **FR-005**: `IRIS_SOURCE_CONTROL` and `IRIS_SKIP_SOURCE_CONTROL` env vars MUST be removed; SCM behavior is driven by per-call tool parameters and elicitation

**Auto-Open**

- **FR-006**: `iris_doc(mode=put)` MUST include `"open_uri": "isfs://NAMESPACE/DocumentName.cls"` in its success response
- **FR-007**: `iris_compile` MUST include `open_uri` in its success response when compiling a single named document
- **FR-008**: The vscode-iris-dev extension MUST watch tool result content for `open_uri` and call VS Code's open document API when an ISFS workspace folder is present
- **FR-009**: Auto-open MUST be silently skipped when no ISFS workspace is open — no error

**SCM Tools**

- **FR-010**: `iris_source_control(action=status)` MUST return editable status, lock owner (if locked), and whether the document is source-controlled
- **FR-011**: `iris_source_control(action=menu)` MUST call IRIS `OnMenu` and return available actions with IDs and labels
- **FR-012**: `iris_source_control(action=execute)` MUST execute the specified action by ID and handle elicitation responses via optional `answer` parameter
- **FR-013**: `iris_source_control(action=checkout)` MUST check out the document if not already checked out and return updated status
- **FR-014**: All `iris_source_control` actions MUST return `{"controlled": false}` gracefully when no SCM system is installed

### Key Entities

- **PendingElicitation**: `{document_name, action_id, context, expires_at}` — session-scoped state tracking in-progress SCM interactions
- **ScmAction**: `{id, label, enabled}` — a single entry from the SCM menu returned by OnMenu
- **ScmStatus**: `{controlled, editable, locked, owner}` — lock/edit state of a document

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A write to a locked document produces an elicitation question in the chat within 2 seconds — not a popup, not a silent failure
- **SC-002**: After answering Yes to a checkout elicitation, the document is saved within 3 seconds
- **SC-003**: After a successful create/write, the document appears open in VS Code within 1 second (when ISFS workspace is active)
- **SC-004**: `iris_source_control(action=menu)` returns the same actions visible in Studio's source control menu for the same document
- **SC-005**: All SCM tool calls return structured responses when no SCM system is installed — no crashes, no unhandled errors
- **SC-006**: Removal of `IRIS_SOURCE_CONTROL`/`IRIS_SKIP_SOURCE_CONTROL` env vars causes no regression in workspaces without source control installed

---

## Clarifications

### Session 2026-04-20

- Q: How should elicitation be implemented given rmcp 1.2 has no first-class Elicitation API? → A: Use MCP spec formal Elicitation message type via raw JSON-RPC (spec-compliant, Option C) as the primary path, with structured JSON response fallback (Option B) when the client signals it does not support elicitation. This makes iris-dev spec-capable while remaining functional for all clients.

---

## Assumptions

1. IRIS UserAction code 1 (Dialog) is the primary case for elicitation — codes 4 (CSP page) and 5 (executable launch) are out of scope for this release; they return descriptive errors.
2. The vscode-iris-dev extension can detect ISFS workspace folders by checking `vscode.workspace.workspaceFolders` for URIs with `isfs://` or `isfs-readonly://` scheme.
3. Tim Leavitt and Nathan Keast will be invited to review this spec before implementation begins.
4. Elicitation implementation: MCP spec formal `elicitation/create` message via raw JSON-RPC as primary path; structured JSON response fallback (`{"elicitation_required": true, "message": "...", "options": [...]}`) when client capability is absent.
5. Session state (PendingElicitation) is in-memory only — not persisted across server restarts.
