---
name: aihub-eap
description: >
  InterSystems AI Hub EAP (Early Access Program) — everything needed to get
  started: download, docker setup, iris key, ConfigStore API, langchain wheel,
  iris-devtester integration, and known build gaps. Load when helping EAP
  participants set up, build, or debug AI Hub projects.
tags: [iris, aihub, eap, ai, configstore, langchain, mcp, docker]
---

# InterSystems AI Hub — EAP Quick Reference

## 1. Getting the Build

**EAP Portal**: https://evaluation.intersystems.com/Eval/early-access/AIHub

Download from the portal:
- `iris-2026.2.0AI.<build>-docker.tar.gz` — IRIS image with AI Hub
- `iris-container-x64.key` — license key (required; community image has connection/data limits without it)
- `langchain_intersystems-0.0.1-py3-none-any.whl` — Python SDK

Current stable EAP build: **2026.2.0AI.141.0**

---

## 2. Docker Setup

### Load and run (with license key)
```bash
# Load the image
docker image load -i iris-2026.2.0AI.141.0-docker.tar.gz

# Run with license key mounted
docker run --name iris-ai-hub \
  -p 1972:1972 \
  -p 52773:52773 \
  -d \
  --volume /path/to/key-dir:/external/keys \
  intersystems/iris:2026.2.0AI.141.0 \
  -k /external/keys/iris-container-x64.key
```

### Without a key (community limits apply)
```bash
docker run --name iris-ai-hub \
  -p 1972:1972 -p 52773:52773 \
  -e IRIS_PASSWORD=SYS \
  -d intersystemsdc/iris-community:2025.1
# No %AI.* classes — need the EAP build
```

### Fix expired default password
```bash
docker exec -it iris-ai-hub iris session iris -U %SYS
# In session:
#  write ##class(Security.Users).UnExpireUserPasswords("*")
```

### iris-devtester integration (EAP participants)
```python
from iris_devtester import IRISContainer, IRISConfig

# Attach to running EAP container
container = IRISContainer.attach("iris-ai-hub")
conn = container.get_connection()

# Or start fresh with a license key
container = (
    IRISContainer("intersystems/iris:2026.2.0AI.141.0")
    .with_name("iris-ai-hub")
    .with_bind_ports(1972, 52773)
    .with_license_key("/path/to/iris-container-x64.key")
    .start()
)
```

---

## 3. %AI.* Class Availability (Build 141.0)

| Class | Purpose |
|-------|---------|
| `%AI.Provider` | LLM provider interface |
| `%AI.Agent` | Execution engine |
| `%AI.Agent.Session` | Session management (nested) |
| `%AI.ToolMgr` | Tool registry & policy |
| `%AI.ToolSet` | Base class for custom toolsets |
| `%AI.Tool` | Base class for plain tool classes |
| `%AI.Tools.SQL` | Built-in SQL tools |
| `%AI.Policy.Discovery` | RAG-based tool selection (experimental) |
| `%AI.Policy.InteractiveAuth` | Auth policy |
| `%AI.Policy.ConsoleAudit` | Audit policy |
| `%AI.MCP.Service` | MCP service dispatch |
| `%AI.LLM.Response` | Response object |
| `%ConfigStore.Configuration` | Config Store API |
| `%Wallet.Collection` | Secret collection management |
| `%Wallet.KeyValue` | Secret key-value storage |

> **Caution**: APIs subject to change before GA. Not for production.

---

## 4. ConfigStore API

`%ConfigStore.Configuration` is the central registry for LLM provider config,
MCP server config, and custom application config.

### Store a configuration
```objectscript
Set config = {
  "type": "AI.LLM",
  "model": "gpt-4o",
  "url": "https://api.openai.com/v1",
  "APIKey": "secret://MySecrets.openai#apikey"
}
Set sc = ##class(%ConfigStore.Configuration).Create(
  "AI", "LLM", "", "openai", config)
```

### Retrieve a configuration
```objectscript
Set cfg = ##class(%ConfigStore.Configuration).Get("AI","LLM","","openai")
// cfg is a %DynamicObject
```

### Delete
```objectscript
Do ##class(%ConfigStore.Configuration).Delete("AI.LLM.openai")
// or with explicit params:
Do ##class(%ConfigStore.Configuration).Delete("AI","LLM","","openai")
```

### Get provider details (with secret resolution)
```objectscript
Set sc = ##class(%ConfigStore.Configuration).GetDetails(
  "AI.LLM.openai", .details, 0, 1)
// details = resolved DynamicObject with secrets substituted
```

### Naming convention
```
Area    = top-level category  (e.g. "AI")
Type    = second-level        (e.g. "LLM", "MCP")
Subtype = third-level         (e.g. "AWSBedrock", "")
Name    = specific instance   (e.g. "openai", "my-server")
Full ID = Area.Type[.Subtype].Name  → "AI.LLM.openai"
```

---

## 5. Wallet (Secret Storage)

Keep API keys out of config by storing them in the Wallet.

```objectscript
// Create a collection
Set perms = {"UseResource": "My.Resource", "EditResource": "My.Resource"}
Do ##class(%Wallet.Collection).Create("MySecrets", perms)

// Store a secret
Set secret = {
  "Usage": "CUSTOM",
  "Secret": {"apikey": "sk-...actual-key..."}
}
Do ##class(%Wallet.KeyValue).Create("MySecrets.openai", secret)

// Reference in ConfigStore config:
// "APIKey": "secret://MySecrets.openai#apikey"
```

```objectscript
// Create the security resource first (%SYS namespace)
Set $NAMESPACE = "%SYS"
Do ##class(Security.Resources).Create("My.Resource")
```

---

## 6. Python (langchain-intersystems) Setup

```bash
python -m venv .venv
source .venv/bin/activate          # macOS/Linux
# .venv\Scripts\activate           # Windows

# Install the wheel from EAP portal
pip install ./langchain_intersystems-0.0.1-py3-none-any.whl

# Install dependencies
pip install mcp langchain-openai langchain-ollama
```

**Common gotchas:**
- Wheel is `py3-none-any` — should install on any platform, but if you hit `platform mismatch` errors, add `--force-reinstall`
- Version number must match the downloaded file exactly — check with `pip show langchain-intersystems`
- If the wheel came from a Windows machine and you're on ARM Mac, the `none-any` tag should be fine — but check if there's a platform-specific build in the portal

```python
from langchain_intersystems.chat_models import init_chat_model
from langchain_intersystems import init_mcp_client

# Initialize with ConfigStore entry
llm = init_chat_model("AI.LLM.openai")
mcp = init_mcp_client("AI.MCP.my-server")
```

---

## 7. AWS Bedrock with Bearer Token

AI Hub EAP typically uses Bedrock via bearer token (ISC SSO).

```objectscript
// ConfigStore config for Bedrock bearer-token mode
Set config = {
  "type": "AI.LLM.AWSBedrock",
  "model": "us.anthropic.claude-sonnet-4-6",
  "region": "us-east-1",
  "bearerToken": "secret://BedkrockSecrets.token#value"
}
```

**Bearer-token Bedrock caveats:**
- Must use cross-region inference profile IDs (e.g. `us.anthropic.claude-sonnet-4-6`) — raw model IDs (`anthropic.claude-sonnet-4-6`) will return 400
- `ListModels()` is NOT supported in bearer-token mode
- SigV4 mode (AWS keys) does not have these restrictions

---

## 8. Known Gaps by Build 141.0

| Gap | Status | Workaround |
|-----|--------|-----------|
| ToolSet definition UI | Forthcoming | Define XML in `XData ToolSet` block |
| MCP Server config via ConfigStore | Forthcoming | Use `iris-mcp-server.config.json` XML |
| Config Store full WIP | Partial | Use `OnInit()` with `GetProviderForConfig()` |
| LangChain4J guide | Forthcoming | Use Python SDK or ObjectScript SDK |
| Smart Discovery (RAG tools) | Experimental | Load `%AI.Policy.Discovery` manually |
| Bedrock `ListModels()` in bearer mode | Not supported | Hardcode model IDs |
| `%AI.Tools.FileSystem` | Rust-based, platform-limited | Use SQL tools or custom ObjectScript tools |

---

## 9. MCP Server (iris-mcp-server)

The MCP server is a separate Rust binary bundled with the EAP.

**Config file**: `iris-mcp-server.config.json`  
**Breaking change at build ~140**: config format changed from v0.1 — if upgrading, regenerate config.

```json
{
  "iris": {
    "host": "localhost",
    "port": 52773,
    "namespace": "USER",
    "username": "_SYSTEM",
    "password": "SYS"
  }
}
```

**Running**:
```bash
iris-mcp-server --config iris-mcp-server.config.json
```

**Claude Desktop / Claude Code setup**:
```json
{
  "mcpServers": {
    "iris-ai-hub": {
      "command": "iris-mcp-server",
      "args": ["--config", "/path/to/iris-mcp-server.config.json"]
    }
  }
}
```

---

## 10. Common First-Session Checklist

1. ✅ Downloaded EAP image + license key from portal
2. ✅ `docker image load` succeeded (image shows in `docker images`)
3. ✅ Container running with `-k /external/keys/iris-container-x64.key`
4. ✅ `http://localhost:52773/csp/sys/UtilHome.csp` loads (Management Portal)
5. ✅ Password un-expired (`UnExpireUserPasswords("*")`)
6. ✅ `##class(%AI.Provider).%IsA("%RegisteredObject")` returns 1 in USER namespace
7. ✅ ConfigStore has at least one LLM provider entry
8. ✅ Wallet has secrets for that provider
9. ✅ `langchain_intersystems` installed and importable
10. ✅ Simple `%AI.Agent` roundtrip produces a response

**Quick smoke test (ObjectScript)**:
```objectscript
Set agent = ##class(%AI.Agent).%New()
Set agent.LLMConfig = "AI.LLM.openai"
Set session = agent.CreateSession()
Set response = session.Ask("What is 2+2?")
Write response.LastMessage
```
