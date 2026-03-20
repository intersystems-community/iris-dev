# iris-dev

Rust CLI and package manager for InterSystems IRIS developer ecosystem.

**Status**: Early development — spec at `022-iris-dev-cli` in objectscript-coder.

## Quick start

```bash
cargo build
# Start MCP server (auto-discovers IRIS)
./target/debug/iris-dev mcp
```

## Commands

- `iris-dev mcp` — Start MCP server for AI coding assistants
- `iris-dev compile [target]` — Compile ObjectScript on IRIS
- `iris-dev install` — Install packages from iris-dev.toml
- `iris-dev --list-plugins` — List iris-dev-* plugins on PATH

## Configuration

Connection discovery cascade:
1. Explicit flags (`--host`, `--web-port`)
2. Env vars (`IRIS_HOST`, `IRIS_WEB_PORT`, `IRIS_USERNAME`, `IRIS_PASSWORD`)
3. VS Code settings.json (`objectscript.conn`)
4. Docker containers (bollard)
5. Localhost scan (ports 52773, 41773, 51773)
