# iris-dev

Rust CLI for AI-assisted InterSystems IRIS development.

Connects AI coding assistants (VS Code Copilot, Cursor, Claude, etc.) to a live IRIS instance via the Model Context Protocol (MCP). Auto-discovers your running IRIS from VS Code settings, Docker, or environment variables.

## Quick start

```bash
cargo build
iris-dev mcp
```

Then in VS Code, add to your MCP servers configuration and start using Copilot agent mode with IRIS-aware tools.

## Commands

- `iris-dev mcp` — Start MCP server (23 IRIS-aware tools)
- `iris-dev compile [target]` — Compile ObjectScript on IRIS
- `iris-dev --list-plugins` — List iris-dev-* plugins on PATH

## Configuration

IRIS connection is auto-discovered in this order:
1. Explicit flags (`--host`, `--web-port`)
2. Env vars (`IRIS_HOST`, `IRIS_WEB_PORT`, `IRIS_USERNAME`, `IRIS_PASSWORD`)
3. VS Code `settings.json` (`objectscript.conn` / `intersystems.servers`)
4. Docker containers (scans for IRIS images)
5. Localhost port scan (52773, 41773, 51773)
