# iris-dev

Connect GitHub Copilot, Claude Code, or any MCP-compatible AI assistant directly to a live InterSystems IRIS instance. Your AI can compile, test, search, read, write, and debug ObjectScript without leaving the chat.

**No Python. No pip. No npm. No API keys.**

---

## How it works

iris-dev runs as a local MCP (Model Context Protocol) server. Your AI assistant calls its tools — `iris_compile`, `iris_doc`, `iris_execute`, etc. — and iris-dev executes them against your real IRIS instance over the Atelier REST API. The AI sees compile errors, class definitions, and execution output in-line, the same way it would with a local filesystem.

---

## Quick start — pick your setup

### Option A: IRIS in Docker (local dev)

```bash
# 1. Install iris-dev (Mac Apple Silicon)
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-macos-arm64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
xattr -d com.apple.quarantine /usr/local/bin/iris-dev 2>/dev/null

# 2. Let iris-dev find your container automatically
iris-dev init              # writes .iris-dev.toml from your running containers

# 3. Add to Claude Code (~/.claude/settings.json)
```
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

### Option B: Remote or server IRIS (no Docker)

```bash
# Set connection via env vars — no .iris-dev.toml needed
```
```json
{
  "mcpServers": {
    "iris-dev": {
      "command": "iris-dev",
      "args": ["mcp"],
      "env": {
        "IRIS_HOST": "iris.example.com",
        "IRIS_WEB_PORT": "52773",
        "IRIS_USERNAME": "_SYSTEM",
        "IRIS_PASSWORD": "SYS",
        "IRIS_NAMESPACE": "MYAPP"
      }
    }
  }
}
```

For HTTPS or a non-root web gateway path:
```json
"IRIS_SCHEME": "https",
"IRIS_WEB_PORT": "443",
"IRIS_WEB_PREFIX": "irisaicore"
```

### Option C: VS Code Copilot Agent Mode

1. Install the binary (see [Installation](#installation) below)
2. Download `vscode-iris-dev-*.vsix` from the [releases page](https://github.com/intersystems-community/iris-dev/releases/latest)
3. In VS Code: Extensions (`Ctrl+Shift+X`) → `...` → **Install from VSIX**
4. Reload VS Code — **iris-dev (IRIS)** appears automatically in Copilot Chat → Agent mode → tools

The extension reads your existing `objectscript.conn` and `intersystems.servers` config — no extra setup if you already use the InterSystems VS Code extensions.

---

## Installation

### Mac

```bash
# Apple Silicon (M1/M2/M3):
sudo mkdir -p /usr/local/bin
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-macos-arm64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
xattr -d com.apple.quarantine /usr/local/bin/iris-dev 2>/dev/null

# Intel Mac: replace "arm64" with "x86_64" above
```

### Linux

```bash
curl -fsSL https://github.com/intersystems-community/iris-dev/releases/latest/download/iris-dev-linux-x86_64 \
  -o /usr/local/bin/iris-dev && chmod +x /usr/local/bin/iris-dev
```

### Windows

1. Download `iris-dev-windows-x86_64.exe` from the [releases page](https://github.com/intersystems-community/iris-dev/releases/latest)
2. Save it somewhere permanent, e.g. `C:\Users\yourname\bin\iris-dev.exe`
3. In VS Code User Settings (JSON), set the binary path:
```json
"iris-dev.serverPath": "C:\\Users\\yourname\\bin\\iris-dev.exe"
```

> **WSL2**: Use the Windows binary. Set `IRIS_HOST` to the Windows host IP — `localhost` in WSL2 resolves to the Linux VM, not the Windows host.

---

## Tools

iris-dev exposes 23 tools to your AI assistant:

| Tool | Needs Docker? | What it does |
|------|:---:|-------------|
| `iris_compile` | — | Compile a class, routine, or wildcard (`MyApp.*.cls`). Returns errors with line numbers. |
| `iris_execute` | — | Run arbitrary ObjectScript and return output. |
| `iris_query` | — | Execute SQL, return rows as JSON. |
| `iris_doc` | — | Read, write, delete, or check any IRIS document. SCM checkout handled via chat dialog. |
| `iris_symbols` | — | Search classes and methods via `%Dictionary`. |
| `docs_introspect` | — | Deep class inspection: methods, properties, XData, superclasses. |
| `iris_search` | — | Full-text search across the namespace. Supports regex and category filters. |
| `iris_info` | — | Namespace discovery: documents, jobs, CSP apps, metadata. |
| `iris_macro` | — | Macro inspection: list, signature, definition, expand. |
| `iris_debug` | — | Map INT errors to source lines, fetch error logs, capture error state. |
| `iris_generate` | — | Build a context-rich prompt for the AI to generate ObjectScript. No API key needed. |
| `iris_generate_class` | — | Generate and compile a class from a description (requires LLM API key). |
| `iris_generate_test` | — | Generate `%UnitTest` scaffolding for an existing class. |
| `iris_source_control` | ✓ | Check lock status, checkout, execute SCM actions. |
| `iris_test` | ✓ | Run `%UnitTest` tests and return structured pass/fail results. |
| `iris_production` | ✓ | Start, stop, update, check, or recover an Interoperability production. |
| `iris_interop_query` | ✓ | Query production logs, queue depths, or message archive. |
| `iris_containers` | ✓ | List, select, or start IRIS Docker containers. |
| `skill` | ✓ | Manage the local skills registry (list, describe, search, forget). |
| `skill_community` | ✓ | Browse community skills. |
| `kb` | ✓ | Index markdown files into a searchable knowledge base. |

Tools marked **✓ Needs Docker** require `IRIS_CONTAINER` to be set. Tools without the mark work over Atelier REST and work with any IRIS instance — local or remote.

---

## Configuration reference

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `IRIS_HOST` | `localhost` | IRIS web gateway hostname |
| `IRIS_WEB_PORT` | `52773` | Web gateway port |
| `IRIS_SCHEME` | `http` | `http` or `https` |
| `IRIS_WEB_PREFIX` | _(empty)_ | URL path prefix (e.g. `irisaicore` for `/irisaicore/api/atelier/`) |
| `IRIS_USERNAME` | `_SYSTEM` | IRIS username |
| `IRIS_PASSWORD` | `SYS` | IRIS password |
| `IRIS_NAMESPACE` | `USER` | Default namespace |
| `IRIS_CONTAINER` | _(empty)_ | Docker container name — required for Docker-dependent tools |
| `OBJECTSCRIPT_WORKSPACE` | `$PWD` | Workspace root for `.iris-dev.toml` lookup |

### `.iris-dev.toml` (per-project config)

Drop this file in your project root and commit it so teammates get the same setup automatically.

```toml
# Local Docker container
container = "myapp-iris"
namespace = "MYAPP"

# Remote IRIS (alternative to Docker)
# host = "iris.example.com"
# web_port = 52773
# scheme = "https"          # for TLS
# web_prefix = "irisaicore" # for non-root gateway path
```

Generate from your running containers: `iris-dev init`

### Connection discovery order

iris-dev resolves the connection in this order — first match wins:

1. CLI flags (`--host`, `--web-port`, `--scheme`)
2. `.iris-dev.toml` in the workspace root
3. Environment variables (`IRIS_HOST`, etc.)
4. VS Code `settings.json` (`objectscript.conn` / `intersystems.servers`)
5. Docker containers (scored by workspace name similarity)
6. Localhost port scan (52773, 41773, 51773, 8080)

### VS Code: Server Manager integration

If you use the InterSystems VS Code extensions, iris-dev reads your server definitions automatically. Your `objectscript.conn` should reference a named server so the full definition (including `pathPrefix` for non-standard gateways) is picked up:

```json
"objectscript.conn": { "active": true, "server": "your-server-name" }
```

If iris-dev can't find your server: `View > Output > iris-dev` shows which servers were found and where.

---

## Commands

```bash
iris-dev mcp           # Start the MCP server (used by Claude Code / Copilot)
iris-dev compile MyApp.Foo.cls   # Compile from the terminal
iris-dev init          # Generate .iris-dev.toml from running containers
iris-dev --version     # Check version
```

---

## Contributing

Issues and PRs welcome. File bugs at the **Issues** tab — visible to the team and helps prioritization.

Questions or urgent issues: [thomas.dyar@intersystems.com](mailto:thomas.dyar@intersystems.com)
