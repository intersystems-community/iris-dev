# iris-dev

**iris-dev** is a single binary that connects GitHub Copilot, Claude Code, and other AI coding assistants directly to a live InterSystems IRIS instance via the Model Context Protocol (MCP). Your AI assistant can compile, test, search, read, write, and debug ObjectScript — all without leaving the chat.

**No Python. No pip. No npm. No API keys.**

For questions, bug reports, or feedback, use the **Issues** tab on this repository, or email [thomas.dyar@intersystems.com](mailto:thomas.dyar@intersystems.com).

---

## Getting Started

Download the binary and VS Code extension directly from the [releases page](https://github.com/intersystems-community/iris-dev/releases/latest).

---

## Installation

Download the binary and VS Code extension from the [latest release](https://github.com/intersystems-community/iris-dev/releases/latest).

### Mac

```bash
# Apple Silicon (M1/M2/M3):
# If /usr/local/bin doesn't exist: sudo mkdir -p /usr/local/bin
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-macos-arm64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
xattr -d com.apple.quarantine /usr/local/bin/iris-dev 2>/dev/null
# Alternative without sudo: install to ~/.local/bin (ensure it's on your PATH)

# Intel Mac:
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-macos-x86_64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
xattr -d com.apple.quarantine /usr/local/bin/iris-dev 2>/dev/null
```

### Linux

```bash
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-linux-x86_64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
```

### Windows

1. Download `iris-dev-windows-x86_64.exe` from the [releases page](https://github.com/intersystems-community/iris-dev/releases/latest) and save it somewhere permanent (e.g. `C:\Users\yourname\bin\iris-dev.exe`)
2. Open VS Code settings (`Ctrl+Shift+P` → "Open User Settings (JSON)") and add:

```json
"iris-dev.serverPath": "C:\\Users\\yourname\\bin\\iris-dev.exe"
```

> **WSL2**: If VS Code runs in WSL2 and IRIS is on the Windows host, use the Windows binary and set `iris-dev.serverPath` to the Windows path. `localhost` in WSL2 resolves to the Linux VM — set `IRIS_HOST` to the Windows host IP instead.

---

## VS Code Setup (Copilot Agent Mode)

1. Download `vscode-iris-dev-*.vsix` from the [latest release](https://github.com/intersystems-community/iris-dev/releases/latest)
2. In VS Code: Extensions panel (`Ctrl+Shift+X`) → `...` menu → **Install from VSIX** → select the file
3. Reload VS Code
4. Open Copilot Chat → Agent mode → click the tools icon (plug) → **iris-dev (IRIS)** should appear

**The extension reads your existing `objectscript.conn` and `intersystems.servers` config automatically** — no duplicate setup. It picks up host, port, namespace, username, password, and `pathPrefix` from your existing Server Manager server definition.

Your `objectscript.conn` should reference a named server (not a direct host/port) so the full server definition is picked up:

```json
"objectscript.conn": {
    "active": true,
    "server": "your-server-name"
}
```

**How server lookup works — important if you have many servers defined:**

The InterSystems Server Manager extension stores server definitions in VS Code **user settings** (global). Some teams also define servers in **workspace settings** (`.vscode/settings.json`). iris-dev reads both and merges them — workspace settings take precedence over user settings when the same server name appears in both.

This means:
- Servers defined only in user settings are found automatically — no extra config needed
- Servers defined in workspace settings override user-level definitions of the same name
- The `server` name in `objectscript.conn` is looked up in the merged result

If iris-dev can't find your named server, open `View > Output > iris-dev` — the log shows which server names were found in user settings vs workspace settings, and which name it was looking for.

> **Non-standard web gateway path**: If your IRIS is served at a URL prefix (e.g. `http://host:80/myprefix/api/atelier/...`), add `"pathPrefix": "myprefix"` to the `webServer` block of your named server in `intersystems.servers`.

> **Troubleshooting**: Open `View > Output` and select **iris-dev** from the dropdown. The log shows exactly what config was read and what env vars were passed to the binary.

---

## Claude Code Setup

Add to `~/.claude/settings.json` (or `.claude/settings.json` in the project root):

```json
{
  "mcpServers": {
    "iris-dev": {
      "type": "stdio",
      "command": "iris-dev",
      "args": ["mcp"],
      "env": {
        "IRIS_HOST": "localhost",
        "IRIS_WEB_PORT": "52773",
        "IRIS_USERNAME": "_SYSTEM",
        "IRIS_PASSWORD": "SYS",
        "IRIS_NAMESPACE": "USER"
      }
    }
  }
}
```

On Windows, use the full path:
```json
"command": "C:\\Users\\yourname\\bin\\iris-dev.exe"
```

**Multiple projects** — use `.iris-dev.toml` in each project root (`iris-dev init` to generate one), then a single generic MCP entry covers all projects:

```json
{
  "mcpServers": {
    "iris-dev": {
      "command": "iris-dev",
      "args": ["mcp"],
      "env": { "OBJECTSCRIPT_WORKSPACE": "${workspaceFolder}" }
    }
  }
}
```

> **Note**: `"args": ["mcp"]` is required. Use `env` not `args` for connection settings.

---

## What iris-dev Does

AI coding assistants assume a local filesystem. IRIS developers don't always have one. iris-dev gives the AI a native IRIS interface — compile, test, and debug against the real live system. The result is fewer compile-fail cycles, fewer hallucinated APIs, and an assistant that actually knows what's in your codebase.

### Tools (21 total)

**Compile, Execute, Test**
| Tool | What it does |
|------|-------------|
| `iris_compile` | Compile a class, routine, or wildcard package (`MyApp.*.cls`). Returns structured errors with line numbers. |
| `iris_execute` | Run arbitrary ObjectScript, return output. |
| `iris_query` | Execute SQL, return rows as JSON. |
| `iris_test` | Run `%UnitTest` tests, return structured pass/fail counts and trace. |

**Documents**
| Tool | What it does |
|------|-------------|
| `iris_doc` | Read, write, delete, or check any IRIS document (`mode=get/put/delete/head`). Write triggers automatic SCM checkout if needed — handled via chat dialog, not a popup. |

**Search & Discovery**
| Tool | What it does |
|------|-------------|
| `iris_search` | Full-text search across the namespace. Supports regex, category filter, wildcard scope. |
| `iris_symbols` | Symbol search via `%Dictionary` — class names, methods, properties. |
| `iris_symbols_local` | Parse local `.cls` files offline (no IRIS required). |
| `iris_introspect` | Deep class inspection: methods, properties, XData, superclasses. |
| `iris_info` | Namespace discovery (`what=documents/modified/namespace/metadata/jobs/csp_apps`). |
| `iris_macro` | Macro inspection (`action=list/signature/location/definition/expand`). |

**Debug**
| Tool | What it does |
|------|-------------|
| `iris_debug` | Map INT errors to source lines, fetch error logs, capture error state. |

**Generate**
| Tool | What it does |
|------|-------------|
| `iris_generate` | Returns a ready-to-use prompt and IRIS context so the AI agent can generate ObjectScript. No API key needed on the server side. |

**Source Control**
| Tool | What it does |
|------|-------------|
| `iris_source_control` | Check lock status, list SCM actions, check out, execute SCM actions. Handles interactive SCM dialogs via chat instead of popups. |

**Interoperability**
| Tool | What it does |
|------|-------------|
| `interop_production` | Start, stop, check status, update, recover productions. |
| `interop_query` | Query logs, queue depths, message archive. |

**Skills & Learning**
| Tool | What it does |
|------|-------------|
| `skill` | Manage the learning agent skills registry. |
| `skill_community` | Browse and install community skills. |
| `kb` | Index markdown files and search the knowledge base. |
| `agent_info` | Session stats and recent tool call history. |

**Containers**
| Tool | What it does |
|------|-------------|
| `iris_containers` | List, select, or start IRIS Docker containers. |

---

## Configuration

IRIS connection is auto-discovered in this order:

1. Explicit flags (`--host`, `--web-port`)
2. `.iris-dev.toml` in the workspace root
3. Env vars: `IRIS_HOST`, `IRIS_WEB_PORT`, `IRIS_USERNAME`, `IRIS_PASSWORD`, `IRIS_NAMESPACE`
4. `IRIS_WEB_PREFIX` — set if IRIS is behind a non-root web gateway (e.g. `"irisaicore"` for `http://host:80/irisaicore/api/atelier`)
5. `IRIS_TOOLSET` — control which tools are registered: `baseline` (all 34, default), `nostub` (29, stubs removed), `merged` (23, consolidated tool set for ablation study)
5. VS Code `settings.json` (`objectscript.conn` / `intersystems.servers` including `pathPrefix`)
6. Docker containers (scored by workspace name similarity)
7. Localhost port scan (52773, 41773, 51773, 8080)

### Per-workspace config (`.iris-dev.toml`)

Drop an `.iris-dev.toml` in your project root. Commit it so teammates get the same connection automatically.

```toml
container = "myapp-iris"   # Docker container name
namespace = "USER"

# Or for remote/non-Docker IRIS:
# host = "iris.example.com"
# web_port = 52773
# web_prefix = ""  # e.g. "irisaicore" if needed
```

Generate a starter file from running containers:
```bash
iris-dev init
```

---

## Commands

- `iris-dev mcp` — Start the MCP server
- `iris-dev compile [target]` — Compile ObjectScript from terminal
- `iris-dev init` — Generate `.iris-dev.toml` from running containers

---

## How to Reach Out

File issues on this repository's **Issues** tab — this makes them visible to the team and helps us prioritize.

For anything that shouldn't be public (credentials, customer details, urgent blockers): email [thomas.dyar@intersystems.com](mailto:thomas.dyar@intersystems.com).

The repo is public — share https://github.com/intersystems-community/iris-dev with anyone who wants to try it.
