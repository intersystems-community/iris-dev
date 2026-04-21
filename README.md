# iris-dev

Single Rust binary that connects AI coding assistants (VS Code Copilot, Claude Code, Cursor, etc.) to a live InterSystems IRIS instance via the Model Context Protocol (MCP).

**No Python. No npm. No API keys.** Drop the binary on your PATH, install the VS Code extension, and your AI assistant can compile, test, search, read, write, and manage ObjectScript — all without leaving the chat.

---

## Quick Start

**Mac / Linux:**
```bash
# Download from OneDrive AI_Platform/iris-dev or build from source
chmod +x iris-dev-macos-arm64
mv iris-dev-macos-arm64 /usr/local/bin/iris-dev

code --install-extension vscode-iris-dev-0.2.0.vsix
```

**Windows:**
```powershell
# Download iris-dev-windows-x86_64.exe and vscode-iris-dev-0.2.0.vsix
Move-Item iris-dev-windows-x86_64.exe "$env:USERPROFILE\bin\iris-dev.exe"
code --install-extension vscode-iris-dev-0.2.0.vsix
```

Reload VS Code. Open Copilot Chat → Agent mode → tools icon → **iris-dev (IRIS)** appears automatically, reading your existing `objectscript.conn`.

If the extension can't find the binary, set in VS Code settings:
```json
"iris-dev.serverPath": "/full/path/to/iris-dev"
```

---

## Tools (21 total — no API key required for any of them)

### Compile, Execute, Test
| Tool | What it does |
|------|-------------|
| `iris_compile` | Compile a class, routine, or wildcard package (`MyApp.*.cls`). Returns structured errors with line numbers. |
| `iris_execute` | Run arbitrary ObjectScript, return output. |
| `iris_query` | Execute SQL, return rows as JSON. |
| `iris_test` | Run `%UnitTest` tests, return structured pass/fail counts and trace. |

### Documents
| Tool | What it does |
|------|-------------|
| `iris_doc` | Read, write, delete, or check any IRIS document (`mode=get/put/delete/head`). Write triggers automatic SCM checkout if needed — handled via chat dialog, not a popup. |

### Search & Discovery
| Tool | What it does |
|------|-------------|
| `iris_search` | Full-text search across the namespace. Supports regex, category filter, wildcard scope. Auto-upgrades to async for large codebases. |
| `iris_symbols` | Symbol search via `%Dictionary` — class names, methods, properties. |
| `iris_symbols_local` | Parse local `.cls` files offline (no IRIS required). |
| `iris_introspect` | Deep class inspection: methods, properties, XData, superclasses. |
| `iris_info` | Namespace discovery (`what=documents/modified/namespace/metadata/jobs/csp_apps`). |
| `iris_macro` | Macro inspection (`action=list/signature/location/definition/expand`). |

### Debug
| Tool | What it does |
|------|-------------|
| `iris_debug` | Map INT errors to source lines, fetch error logs, capture error state (`action=map_int/error_logs/capture/source_map`). |

### Generate
| Tool | What it does |
|------|-------------|
| `iris_generate` | Returns a ready-to-use prompt and IRIS context (existing class names, method signatures) so the AI agent can generate an ObjectScript class or `%UnitTest` itself. No API key needed — the calling agent does the generation. |

### Source Control
| Tool | What it does |
|------|-------------|
| `iris_source_control` | Check lock status, list available SCM actions, check out, execute SCM actions (`action=status/menu/checkout/execute`). Handles interactive SCM dialogs via chat instead of popups. |

### Interoperability
| Tool | What it does |
|------|-------------|
| `interop_production` | Start, stop, check status, update, recover productions (`action=status/start/stop/update/needs_update/recover`). |
| `interop_query` | Query logs, queue depths, message archive (`what=logs/queues/messages`). |

### Skills & Learning
| Tool | What it does |
|------|-------------|
| `skill` | Manage the learning agent skills registry (`action=list/describe/search/forget/propose`). |
| `skill_community` | Browse and install community skills (`action=list/install`). |
| `kb` | Index markdown files and search the knowledge base (`action=index/recall`). |
| `agent_info` | Session stats and recent tool call history (`what=stats/history`). |

### Containers
| Tool | What it does |
|------|-------------|
| `iris_containers` | List, select, or start IRIS Docker containers (`action=list/select/start`). |

---

## Configuration

IRIS connection auto-discovered in this order:
1. Explicit flags (`--host`, `--web-port`)
2. Env vars: `IRIS_HOST`, `IRIS_WEB_PORT`, `IRIS_USERNAME`, `IRIS_PASSWORD`, `IRIS_NAMESPACE`
3. `IRIS_WEB_PREFIX` — set this if your IRIS is behind a non-root web gateway (e.g. `irisaicore` for `http://host:80/irisaicore/api/atelier`)
4. VS Code `settings.json` (`objectscript.conn` / `intersystems.servers` including `pathPrefix`)
5. Docker containers (scored by workspace name similarity)
6. Localhost port scan (52773, 41773, 51773, 8080)

### Source Control

When writing documents with `iris_doc(mode=put)`, if IRIS server-side source control is enabled:
- The tool automatically checks whether the document needs checkout
- If a dialog is needed, it returns a chat question instead of a popup
- You answer yes/no in the chat, and the write completes

No `IRIS_SOURCE_CONTROL` env var needed — it's detected automatically.

### Auto-open in VS Code

After a successful `iris_doc(mode=put)` or `iris_compile`, the document opens automatically in VS Code if an ISFS workspace folder is active. This uses a sentinel file at `~/.iris-dev/open-hint.json` watched by the extension.

---

## Build from source

```bash
git clone https://gitlab.iscinternal.com/devx/iris-dev
cd iris-dev
cargo build --release
# Binary at: target/release/iris-dev
```

Requires Rust stable. No other dependencies.

---

## Working with multiple IRIS instances or namespaces

**Different instances** — set `IRIS_HOST` + `IRIS_WEB_PORT` per project. The VS Code extension reads these from your workspace's `objectscript.conn` automatically. For Claude Code, set them in `.claude/settings.json`:

```json
{
  "mcpServers": {
    "iris-dev": {
      "command": "/path/to/iris-dev",
      "env": {
        "IRIS_HOST": "prod-iris.example.com",
        "IRIS_WEB_PORT": "52773",
        "IRIS_WEB_PREFIX": "myapp",
        "IRIS_USERNAME": "devuser",
        "IRIS_PASSWORD": "devpass",
        "IRIS_NAMESPACE": "MYAPP"
      }
    }
  }
}
```

**Limiting to a specific namespace** — set `IRIS_NAMESPACE`. All tools (`iris_symbols`, `iris_search`, `iris_info`, `iris_compile`, etc.) scope to this namespace by default. Each tool also accepts an explicit `namespace` parameter to override per-call. This keeps the context window small — `iris_symbols` only searches your namespace, not all of `%SYS`.

**Non-root web gateway** — set `IRIS_WEB_PREFIX` to the path prefix (e.g. `"irisaicore"` for `http://host:80/irisaicore/api/atelier`).

---

## Commands

- `iris-dev mcp` — Start the MCP server
- `iris-dev compile [target]` — Compile ObjectScript directly from terminal
- `iris-dev --list-plugins` — List iris-dev-* plugins on PATH
