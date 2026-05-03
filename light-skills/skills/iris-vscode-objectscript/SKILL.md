---
name: iris-vscode-objectscript
description: >
  Use when configuring VSCode for ObjectScript development against an IRIS
  container. Covers the intersystems.servers settings.json config, the
  critical port 52773 vs 1972 distinction, why enterprise images don't work
  directly, webgateway limitations, and how to verify the connection.
  Load when: setting up iris-dev VSCode extension, getting 404 on /api/atelier/,
  or choosing between enterprise vs community for local development.
tags: [iris, vscode, objectscript, atelier, webserver, devtester, extension]
---

# VSCode ObjectScript Extension — IRIS Setup

## HARD GATE — Read Before Touching docker-compose or webgateway

The VSCode `intersystems.objectscript` extension uses **Atelier REST** (`/api/atelier/`).
Atelier REST is served by IRIS's **internal HTTP process on port 52773**.
It is NOT served by the superserver on port 1972.

**There is no way to make this work with enterprise images that have `WebServer=0`.**
A webgateway proxies CSP protocol to the superserver — it cannot route REST requests to a process that isn't running.

### Which Images Have Port 52773?

| Image | Has web server | VSCode works |
|---|---|---|
| `intersystemsdc/iris-community:*` | ✅ | ✅ |
| `intersystemsdc/irishealth-community:*` | ✅ | ✅ |
| `containers.intersystems.com/intersystems/iris:*` (enterprise) | ❌ | ❌ direct |
| `irishealth:2026.2.0AI.*` (AI Hub) | ❌ | ❌ direct |

**For development: use `iris-community:YYYY.N` — identical ObjectScript/SQL/globals, same version.**

---

## settings.json Configuration

```jsonc
// macOS: ~/Library/Application Support/Code/User/settings.json
// Windows: %APPDATA%\Code\User\settings.json
// Linux: ~/.config/Code/User/settings.json
{
  "intersystems.servers": {
    "my-iris": {
      "webServer": {
        "host": "localhost",
        "port": 52773,         // IRIS private web server — NOT 1972
        "pathPrefix": ""
      },
      "username": "_SYSTEM",
      "description": "iris-community:2026.1"
    }
  },
  "objectscript.conn": {
    "server": "my-iris",
    "ns": "USER",             // namespace to edit in
    "active": true
  }
}
```

**If docker maps 52773 to a different host port** (e.g. `"64773:52773"`):
```jsonc
"port": 64773     // use the HOST port, not the container port
```

---

## Verify Before Opening VSCode

```bash
# Confirm Atelier REST is responding
curl -s -u "_SYSTEM:SYS" "http://localhost:52773/api/atelier/" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['content']['version'])"
# → IRIS for UNIX (Apple M3 Ultra) 2026.1 (Build 123U) ...

# If 404: IRIS web server isn't running (enterprise image, or IRIS not fully started)
# If 401: wrong credentials
# If connection refused: wrong port or container not running
```

---

## Common docker-compose Patterns

### Community (recommended for VSCode dev)

```yaml
services:
  iris:
    image: intersystemsdc/iris-community:2026.1
    container_name: iris-dev
    ports:
      - "1972:1972"    # superserver (DBAPI/JDBC)
      - "52773:52773"  # web server (VSCode/Atelier REST)
    environment:
      - ISC_CPF_MERGE_FILE=/tmp/merge.cpf
    volumes:
      - ./merge.cpf:/tmp/merge.cpf:ro
```

```
# merge.cpf
[Actions]
ModifyService:Name=%Service_CallIn,Enabled=1,AutheEnabled=48
ModifyUser:Name=_SYSTEM,ChangePassword=0,PasswordNeverExpires=1
ModifyUser:Name=SuperUser,ChangePassword=0,PasswordNeverExpires=1
```

### Enterprise + Community (two-container for enterprise features + VSCode)

```yaml
services:
  iris-enterprise:
    image: containers.intersystems.com/intersystems/iris:2026.1
    container_name: iris-enterprise
    ports:
      - "4972:1972"    # superserver only — NO web server
    volumes:
      - ./iris.key:/usr/irissys/mgr/iris.key:ro

  iris-dev:
    image: intersystemsdc/iris-community:2026.1
    container_name: iris-community-dev
    ports:
      - "1972:1972"
      - "52773:52773"  # ← VSCode connects here
```

VSCode settings.json uses `iris-community-dev` on port 52773.
Enterprise-specific tests connect to `iris-enterprise` on port 4972 via DBAPI.

---

## iris-devtester Integration

```python
from iris_devtester import IRISContainer

# community() exposes both ports — VSCode works immediately after start()
with IRISContainer.community() as iris:
    web_port = iris.get_mapped_port(52773)
    print(f"VSCode port: {web_port}")
    # Add to settings.json: "port": web_port
```

---

## What the Webgateway CAN'T Do

The ISC webgateway (`containers.intersystems.com/intersystems/webgateway:*`) proxies:
- ✅ CSP applications (Management Portal, legacy CSP pages)
- ✅ `/csp/bin/Systems/` management endpoints
- ❌ `/api/atelier/` — Atelier REST
- ❌ Any REST endpoint served by IRIS's internal HTTP process

Even with correct Apache `<Location>` blocks and CSP.ini `/api` entries, the CSP module returns 404 because the IRIS superserver doesn't handle REST routing. This is a 30-minute rabbit hole. Don't go down it.

---

## Namespace Switching in VSCode

Change `objectscript.conn.ns` in settings.json, or use the namespace picker in the VSCode status bar (bottom left, shows current namespace).

To compile into a specific namespace from the CLI:

```bash
# iris-devtester
from iris_devtester.containers.iris_container import IRISContainer
iris.execute_objectscript("Do $SYSTEM.OBJ.Load('/path/to/file.cls', 'ck')", namespace="MYNS")
```
