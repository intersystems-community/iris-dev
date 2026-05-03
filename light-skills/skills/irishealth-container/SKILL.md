---
name: irishealth-container
description: >
  Use when working with irishealth-community (FHIR R4) or irishealth AI Hub
  containers, OR when setting up VSCode/iris-dev extension against any IRIS
  container. Covers IRISContainer.health() and ai_hub() factory methods,
  four hard-won gotchas, the two-container architecture, no-ZPM FHIR setup,
  and the critical enterprise-vs-community web server split that affects
  Atelier REST and VSCode ObjectScript extension connectivity.
tags: [iris, irishealth, fhir, ai-hub, docker, container, devtester, vscode, atelier]
---

# irishealth Container Editions

---

## CRITICAL: Enterprise Images Have No Web Server — Atelier REST Won't Work

**This burned 30 minutes in a session. Stop here before trying to set up a webgateway.**

| Image | `WebServer` in iris.cpf | Port 52773 | Atelier REST | VSCode ObjectScript |
|---|---|---|---|---|
| `iris:2026.1` (enterprise) | `0` — disabled | ❌ | ❌ | ❌ direct |
| `iris-community:2026.1` | `1` — enabled | ✅ | ✅ | ✅ |
| `irishealth-community` | `1` — enabled | ✅ | ✅ | ✅ |
| `irishealth:2026.2.0AI.*` | `0` — disabled | ❌ | ❌ | ❌ direct |

**Why a webgateway won't help:**
- The ISC webgateway proxies **CSP protocol** → IRIS superserver (port 1972)
- Atelier REST (`/api/atelier/`) is served by IRIS's **internal HTTP process** (port 52773)
- The superserver does not speak HTTP. There is no way to proxy REST through it.
- Even with correct Apache `<Location>` + CSP.ini `/api` entries, the CSP module returns 404 because IRIS's superserver doesn't handle REST routing

**The solution: use community edition of the same IRIS version**

Enterprise and community editions are identical for development purposes (same ObjectScript, SQL, globals). The only differences: enterprise supports mirroring/sharding/licensing features you don't need for dev.

```
iris-community:2026.1  →  port 52773 available  →  VSCode works
iris:2026.1            →  port 52773 unavailable →  VSCode doesn't work directly
```

**If you genuinely need enterprise features AND VSCode**: use a two-container setup — enterprise on superserver port, community on web port — and develop against community while testing enterprise-specific features separately.

---

## VSCode ObjectScript Extension — Server Configuration

The `intersystems.objectscript` extension connects via Atelier REST on port 52773 (the IRIS private web server), **not** the superserver port 1972.

```jsonc
// ~/Library/Application Support/Code/User/settings.json (macOS)
// %APPDATA%\Code\User\settings.json (Windows)
{
  "intersystems.servers": {
    "my-iris-dev": {
      "webServer": {
        "host": "localhost",
        "port": 52773,       // ← IRIS private web server, NOT superserver
        "pathPrefix": ""     // empty for default, "/iris" if behind a proxy
      },
      "username": "_SYSTEM",
      "description": "iris-community:2026.1 dev container"
    }
  },
  "objectscript.conn": {
    "server": "my-iris-dev",
    "ns": "USER",
    "active": true
  }
}
```

If your container maps 52773 to a different host port (e.g. `"64773:52773"`), use the host port in settings:

```jsonc
"webServer": {
  "host": "localhost",
  "port": 64773    // ← whatever the docker -p maps 52773 to on the host
}
```

**Verify the connection works before opening VSCode:**

```bash
curl -s -u "_SYSTEM:SYS" "http://localhost:64773/api/atelier/" | python3 -c \
  "import sys,json; d=json.load(sys.stdin); print(d['result']['content']['version'])"
# → IRIS 2026.1 (or similar) confirms Atelier REST is working
```

---

## Two Editions, Two Jobs

| | `IRISContainer.health()` | `IRISContainer.ai_hub()` |
|---|---|---|
| Image | `intersystemsdc/irishealth-community:latest` | `docker.iscinternal.com/.../irishealth:2026.2.0AI.159.0` |
| Port 1972 | ✅ SuperServer | ✅ SuperServer |
| Port 52773 | ✅ FHIR HTTP / web portal | ❌ WebServer=0 |
| FHIR R4 endpoint | ✅ pre-installed | ❌ (class exists, no HTTP) |
| `%AI.Agent`, `VECTOR` | ❌ | ✅ |
| License key | No | No |
| Registry | docker.io | docker.iscinternal.com |

---

## Factory Methods

```python
from iris_devtester import IRISContainer

# FHIR R4 — Foundation.Install + InstallInstance baked at build time
# No ZPM, no network. Ready in ~90 seconds.
with IRISContainer.health() as iris:
    h = iris.fhir_health_check()
    print(h.report())
    # Endpoint: http://localhost:52773/csp/healthshare/demo/fhir/r4
    # FHIR: R4 v4.0.1 — 148 resource types

# AI Hub — %AI.Agent, %AI.MCP.Service, VECTOR SQL, SuperServer only
# Requires docker.iscinternal.com registry access
with IRISContainer.ai_hub(build="159") as iris:
    conn = iris.get_connection()   # port 1972 only
    # iris.cls("%AI.Agent") accessible
```

---

## FHIR Setup (No ZPM, No Network)

The `health()` container has this baked at build time:

```objectscript
set $NAMESPACE = "HSLIB"
do:'##class(%SYS.Namespace).Exists("demo") ##class(HS.Util.Installer.Foundation).Install("demo")
set $NAMESPACE = "demo"
set appKey = "/csp/healthshare/demo/fhir/r4"
set strategyClass = "HS.FHIRServer.Storage.Json.InteractionsStrategy"
set metadataPackages = $LISTBUILD("hl7.fhir.r4.core@4.0.1","hl7.fhir.us.core@3.1.0")
do ##class(HS.FHIRServer.Installer).InstallInstance(appKey, strategyClass, metadataPackages)
```

~30 seconds, zero network calls.

---

## 4 Hard-Won Gotchas

### Gotcha 1: `/durable` Volume Ownership

Named Docker volumes mount as `root`. `irisowner` (uid 51773) cannot write.
`ai_hub()` defaults to `tmpfs:/durable`. For persistence, pass `durable_path`:

```python
iris = IRISContainer.ai_hub(durable_path="/host/path/durable")
# Host path must be pre-chowned: chown 51773:51773 /host/path/durable
```

docker-compose init-container pattern (from grongierisc template):
```yaml
services:
  init-permissions:
    image: alpine:latest
    volumes: [iris-data:/dur]
    command: sh -c "chown -R 51773:51773 /dur"
  iris:
    depends_on:
      init-permissions:
        condition: service_completed_successfully
volumes:
  iris-data:
```

### Gotcha 2: Double-Start Bug

The `-a` hook in `/iris-main` runs **after** IRIS is already started. Any startup script under `-a` that calls `iris start IRIS quietly` will fail with "database already running" and exit. Scripts under `-a` must assume IRIS is live — poll port 1972, do not call `iris start`.

### Gotcha 3: No Web Server in Enterprise Image

`irishealth:2026.2.0AI.*` has `WebServer=0` in `iris.cpf`. No `csp/bin/` directory. Port 52773 not available. FHIR HTTP endpoint not reachable. For both `%AI.*` and FHIR HTTP: use two-container split.

### Gotcha 4: `%AI.*` in Read-Only irislib

`%AI.Agent`, `%AI.MCP.Service`, `%AI.ToolSet`, `%AI.Policy.Authorization` live in `irislib` — a read-only database. Cannot export as UDL/XML. Cannot transplant to community image.

---

## Two-Container Split (docker-compose)

```yaml
services:
  fhir:
    image: intersystemsdc/irishealth-community:latest
    ports: ["1972:1972", "52773:52773"]
    # FHIR R4: http://localhost:52773/csp/healthshare/demo/fhir/r4

  ai_hub:
    image: docker.iscinternal.com/docker-intersystems/intersystems/irishealth:2026.2.0AI.159.0
    ports: ["11972:1972"]
    volumes:
      - type: tmpfs
        target: /durable
        tmpfs: {uid: 51773, gid: 51773}
    # %AI.Agent: connect to localhost:11972 via DBAPI
```

---

## FHIRContainerHealth

```python
from iris_devtester.containers.models import FHIRContainerHealth

h = iris.fhir_health_check()
h.fhir_version       # "4.0.1"
h.endpoint           # "http://localhost:52773/csp/healthshare/demo/fhir/r4"
h.resource_types_count  # 148
h.ready              # True if accessible and fhir_version set
h.report()           # human-readable summary
```
