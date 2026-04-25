---
name: aihub-eap
description: >
  InterSystems AI Hub EAP (Early Access Program) — accurate API patterns for
  builds 158/159 (current). Covers %AI.Agent declarative Parameters, %AI.Provider.Create,
  ConfigStore/GetProviderForConfig, @{env/config/wallet} substitution, session
  management, streaming, tool sets, and known breaking changes from build 141.
  Load when helping EAP participants set up, build, or debug AI Hub projects.
tags: [iris, aihub, eap, ai, configstore, langchain, mcp, docker]
---

# InterSystems AI Hub — EAP Reference (Build 159)

> **Current build: 2026.2.0AI.158** — APIs changed significantly from build 141.
> `LLMConfig` property is GONE. The pattern is now Parameters + `%Init()` + ConfigStore.

---

## 1. Getting the Build

**EAP Portal**: https://evaluation.intersystems.com/Eval/early-access/AIHub

**Current build: 2026.2.0AI.158.0** (as of 2026-04-25)

Available downloads:
```
iris-community-2026.2.0AI.158.0-docker.tar.gz          # x86_64 Docker (community, no key needed)
iris_arm64-community-2026.2.0AI.158.0-docker.tar.gz    # ARM64 Docker (community)
iris-container-x64.key                                   # License key (removes community limits)
iris-container-arm64.key                                  # ARM64 license key
iris.key                                                  # General license key
langchain_intersystems-0.0.1-py3-none-any.whl           # Python SDK
IRIS_Community-2026.2.0AI.158.0-macx64.tar.gz           # macOS Intel
IRIS_Community-2026.2.0AI.158.0-win_x64.exe             # Windows
```

**Community builds** (`iris-community-*`) run without a license key but have connection/data limits. Use a key to remove limits — request from the EAP portal or from your ISC contact.

---

## 2. Docker Setup

```bash
docker image load -i iris-2026.2.0AI.159-docker.tar.gz

docker run --name iris-ai-hub \
  -p 1972:1972 -p 52773:52773 \
  -d \
  --volume /path/to/key-dir:/external/keys \
  intersystems/iris:2026.2.0AI.159 \
  -k /external/keys/iris-container-x64.key
```

### Fix expired default password
```bash
docker exec -it iris-ai-hub iris session iris -U %SYS
# In session:
#  write ##class(Security.Users).UnExpireUserPasswords("*")
```

### iris-devtester integration
```python
from iris_devtester import IRISContainer
container = IRISContainer("intersystems/iris:2026.2.0AI.159") \
    .with_name("iris-ai-hub") \
    .with_bind_ports(1972, 52773) \
    .with_license_key("/path/to/iris-container-x64.key") \
    .start()
```

---

## 3. %AI.Agent — Build 159 Patterns

### Pattern A: Declarative subclass (recommended)

```objectscript
Class MyApp.AI.MyAgent Extends %AI.Agent
{
    Parameter PROVIDER = "openai";           // provider name
    Parameter MODEL = "gpt-4o";             // model id
    Parameter APIKEY;                        // empty = read from env OPENAI_API_KEY
    Parameter TOOLSETS = "%AI.Tools.SQL";   // comma-separated toolsets

    XData INSTRUCTIONS [ MimeType = text/markdown ]
    {
    You are a helpful IRIS database assistant.
    Use SQL tools to answer questions about the data.
    }

    Method %OnInit() As %Status
    {
        // Optional: additional setup after %Init() completes
        // Set ..Temperature = 0.7
        Return $$$OK
    }
}
```

**Usage:**
```objectscript
Set agent = ##class(MyApp.AI.MyAgent).%New()
$$$ThrowOnError(agent.%Init())           // REQUIRED — initializes provider, tools, prompt

Set session = agent.CreateSession()
Set response = agent.Chat(session, "How many patients are in the database?")
Write response.Content
```

> **DO NOT** call `Chat()` without `%Init()` first — tools and provider won't be wired.

### Pattern B: Programmatic (for dynamic provider selection)

```objectscript
Set settings = {}
Do settings.%Set("api_key", apiKey)
Set provider = ##class(%AI.Provider).Create("openai", settings)

Set agent = ##class(%AI.Agent).%New(provider)
Set agent.Model = "gpt-4o-mini"
Set agent.SystemPrompt = "You are a helpful assistant."

Set session = agent.CreateSession()
Set response = agent.Chat(session, "Hello!")
Write response.Content
```

### Pattern C: ConfigStore integration (production pattern)

```objectscript
Class MyApp.AI.ProdAgent Extends %AI.Agent
{
    Parameter MODELCONFIGNAME = "opsreview";   // name in ConfigStore

    Method %OnInit() As %Status
    {
        Set sc = $$$OK
        Try {
            If ..Provider = "" && ..#MODELCONFIGNAME '= "" {
                Set sc = ..GetProviderForConfig(..#MODELCONFIGNAME, .provider, .model)
                Quit:$$$ISERR(sc)
                Set ..Provider = provider
                Set ..Model = model
            }
        } Catch ex {
            Set sc = ex.AsStatus()
        }
        Return sc
    }

    ClassMethod GetProviderForConfig(
        configName As %String,
        Output provider As %AI.Provider,
        Output model As %String) As %Status
    {
        Set sc = $$$OK
        Try {
            Set sc = ##class(%ConfigStore.Configuration).GetDetails(
                "AI.LLM."_configName, .details, 0, 1)
            Quit:$$$ISERR(sc)
            Set provider = ##class(%AI.Provider).Create(details."model_provider", details)
            Set model = details."model"
        } Catch ex {
            Set sc = ex.AsStatus()
        }
        Quit sc
    }
}
```

**ConfigStore entry for "opsreview":**
```objectscript
Set config = {
    "model_provider": "anthropic",
    "model": "claude-sonnet-4-5@20250929",
    "api_key": "secret://MySecrets.anthropic#apikey"
}
Do ##class(%ConfigStore.Configuration).Create("AI","LLM","","opsreview", config)
```

---

## 4. @{} Variable Substitution

Used inside `PROVIDERCONFIG` parameters and ToolSet XML — NOT in ObjectScript code directly.

| Syntax | Source | Example |
|--------|--------|---------|
| `@{env.VAR}` | OS environment variable | `@{env.OPENAI_API_KEY}` |
| `@{config.Key}` | `^%AI.Config` global | `@{config.VertexSAPath}` |
| `@{wallet.Col.Key}` | IRIS Secure Wallet | `@{wallet.AISecrets.anthropic}` |

```objectscript
// In a declarative agent Parameter:
Parameter PROVIDERCONFIG = "{
    ""project_id"": ""my-gcp-project"",
    ""region"": ""us-east5"",
    ""service_account_path"": ""@{env.VERTEX_SA_PATH}""
}";

// In ToolSet XData (MCP remote server with token from wallet):
<MCP Name="MyServer">
    <Remote URL="https://mcp.example.com/mcp"
            AuthType="bearer"
            Token="@{wallet.MCPSecrets.token}"/>
</MCP>
```

> `@{config.AI.LLM.opsreview.APIKey}` is NOT a valid pattern.
> Use `GetDetails()` + `%AI.Provider.Create()` for ConfigStore-backed API keys in code.

---

## 5. Session Management

```objectscript
// Create session (inherits agent's provider, model, prompt, tools)
Set session = agent.CreateSession()

// With config overrides
Set cfg = {"max_iterations": 10, "temperature": 0.7, "max_tokens": 1000}
Set session = agent.CreateSession(cfg)

// Blocking chat
Set response = agent.Chat(session, "Your question here")
Write response.Content

// Streaming
Set renderer = ##class(%AI.System.StreamRenderer).%New()
Set response = agent.StreamChat(session, "Your question", renderer, "OnChunk")
Do renderer.Flush()

// Multi-modal
Set content = [
    {"type": "text", "text": "What is in this image?"},
    {"type": "image_url", "image_url": {"url": "https://example.com/img.jpg"}}
]
Set response = agent.ChatWithContent(session, content)

// Session stats
Set stats = session.GetStats()
Write stats."total_interactions", " turns, ", stats."total_tool_calls", " tool calls"

// Context inspection
Set messages = session.GetContext()
Set iter = messages.%GetIterator()
While iter.%GetNext(.i, .msg) { Write msg.role, ": ", $EXTRACT(msg.content,1,80), ! }

// Session control
Do session.Reset()           // full reset
Do session.ResetContext()    // messages only
Do session.ResetStats()      // stats only
```

---

## 6. %AI.Provider — Supported Providers

```objectscript
// OpenAI
Set provider = ##class(%AI.Provider).Create("openai",
    {"api_key": key, "organization": orgId})

// Anthropic
Set provider = ##class(%AI.Provider).Create("anthropic", {"api_key": key})

// AWS Bedrock — bearer token (ISC SSO)
Set provider = ##class(%AI.Provider).Create("bedrock",
    {"region": "us-east-1", "bearer_token": token})
// MUST use cross-region inference IDs: "us.anthropic.claude-sonnet-4-6"
// NOT raw model IDs: "anthropic.claude-sonnet-4-6"

// AWS Bedrock — SigV4 (AWS keys)
Set provider = ##class(%AI.Provider).Create("bedrock",
    {"region": "us-east-1"})   // reads AWS_ACCESS_KEY_ID etc from env

// Google Vertex AI
Set provider = ##class(%AI.Provider).Create("vertex",
    {"project_id": pid, "region": "us-east5",
     "service_account_path": saPath})

// Google Gemini
Set provider = ##class(%AI.Provider).Create("gemini", {"api_key": key})

// xAI Grok
Set provider = ##class(%AI.Provider).Create("grok", {"api_key": key})

// NVIDIA NIM
Set provider = ##class(%AI.Provider).Create("nim", {"base_url": url})
```

---

## 7. ConfigStore API

```objectscript
// Store
Set config = {"model_provider":"anthropic","model":"claude-sonnet-4-5@20250929",
              "api_key":"secret://Secrets.ant#apikey"}
Do ##class(%ConfigStore.Configuration).Create("AI","LLM","","myconfig", config)

// Retrieve (raw)
Set cfg = ##class(%ConfigStore.Configuration).Get("AI","LLM","","myconfig")

// Retrieve with secret resolution (resolveSecrets=1)
Set sc = ##class(%ConfigStore.Configuration).GetDetails(
    "AI.LLM.myconfig", .details, 0, 1)
// details."model_provider", details."model", details."api_key" all resolved

// Delete
Do ##class(%ConfigStore.Configuration).Delete("AI.LLM.myconfig")
```

---

## 8. Wallet (Secrets)

```objectscript
// Create security resource first (%SYS namespace)
Set $NAMESPACE = "%SYS"
Do ##class(Security.Resources).Create("AI.Secrets")

// Create collection
Do ##class(%Wallet.Collection).Create("AISecrets",
    {"UseResource": "AI.Secrets", "EditResource": "AI.Secrets"})

// Store secret
Do ##class(%Wallet.KeyValue).Create("AISecrets.anthropic",
    {"Usage": "CUSTOM", "Secret": {"apikey": "sk-ant-...actual-key..."}})

// Reference in ConfigStore: "api_key": "secret://AISecrets.anthropic#apikey"
```

---

## 9. Python (langchain-intersystems)

```bash
python -m venv .venv && source .venv/bin/activate
pip install ./langchain_intersystems-0.0.1-py3-none-any.whl
pip install mcp langchain-openai langchain-ollama
```

```python
from langchain_intersystems.chat_models import init_chat_model
from langchain_intersystems import init_mcp_client

llm = init_chat_model("AI.LLM.myconfig")    # uses ConfigStore entry
mcp = init_mcp_client("AI.MCP.my-server")
```

**Common issues:**
- Version mismatch: check `pip show langchain-intersystems` matches the whl filename
- Platform errors: add `--force-reinstall` if wheel metadata conflicts

---

## 10. Breaking Changes: Build 141 → 159

| Feature | Build 141 | Build 159 |
|---------|-----------|-----------|
| `LLMConfig` property | Existed | **REMOVED** |
| Agent instantiation | `##class(%AI.Agent).%New(provider)` only | Also: declarative Parameters subclass |
| Config Store | Partial WIP | Full `%ConfigStore.Configuration` support |
| `%Init()` requirement | Optional | **REQUIRED** before first `Chat()` |
| Streaming | Not available | `StreamChat()` + `%AI.System.StreamRenderer` |
| Session reset | Not available | `Reset()`, `ResetContext()`, `ResetStats()` |
| Checkpoints | Not available | `AddCheckpoint()`, `RewindTo()` |
| Session fork | Not available | `Fork()`, `ForkAndSummarize()` |
| Nested agents | Not available | `CreateSubAgent()`, `DelegateTask` tool |
| Skills | Not available | `%AI.Agent.Skill` with XData |
| RAG | Not available | `%AI.KnowledgeBase`, `EnableSmartDiscovery()` |
| Prompt caching | Not available | `"cache": {"enabled": 1}` in session config |
| `@{wallet.*}` substitution | Not available | `@{wallet.Collection.Key}` |

**DEAD CODE from build 141 (do not use):**
```objectscript
// ❌ LLMConfig is gone
Set agent.LLMConfig = "AI.LLM.openai"

// ❌ Must call %Init() first
Set agent = ##class(MyAgent).%New()
Set response = agent.Chat(session, "hi")   // will fail — no provider wired
```

---

## 11. Quick Smoke Test

```objectscript
// Verify %AI.* classes are available
Write ##class(%AI.Provider).%IsA("%RegisteredObject"), !   // → 1

// Minimal working agent (env var for key)
Set env("OPENAI_API_KEY") = "sk-..."      // or set in OS env before starting IRIS
Set provider = ##class(%AI.Provider).Create("openai", {"api_key": $SYSTEM.Util.GetEnviron("OPENAI_API_KEY")})
Set agent = ##class(%AI.Agent).%New(provider)
Set agent.Model = "gpt-4o-mini"
Set session = agent.CreateSession()
Set r = agent.Chat(session, "Say hi")
Write r.Content
```

---

## 12. Verified-on-Build-159 Findings (2026-04-25)

### `@{}` substitution — `wallet` prefix NOT registered on 159

Only `env` and `config` prefixes work. `wallet` causes an error at runtime.

```objectscript
// ✅ Works on 159:
Parameter APIKEY = "@{env.OPENAI_API_KEY}";

// ❌ Fails on 159 — wallet not registered:
Parameter APIKEY = "@{wallet.AISecrets.openai}";
```

Use env vars or store keys directly in `PROVIDERCONFIG` referencing `@{env.*}`.

### `%AI.LLM.Response` — use `.Content`, never `.%Get("content")`

`response.Content` is a typed `%String` property. `response.%Get()` throws `<METHOD DOES NOT EXIST>`.

```objectscript
// ✅ Correct:
Write response.Content

// ❌ Throws <METHOD DOES NOT EXIST>:
Write response.%Get("content")
```

Available properties on the response object:
- `.Content` — `%String` — the assistant's text reply
- `.ToolCalls` — `%DynamicArray` — tool call requests from the model
- `.Usage` — `%DynamicObject` — token usage (`prompt_tokens`, `completion_tokens`)

### `LLMConfig` property is gone on 159 (confirmed)

The `LLMConfig` property documented for build 141 does NOT exist on 159.
Using it causes `<PROPERTY DOES NOT EXIST>`. Use declarative Parameters or programmatic `%AI.Provider.Create()`.

### `irishealth` vs `iris` image — use `irishealth` for MCP over HTTP

| Image | WebServer | CSPServer binary | Use for |
|-------|-----------|-----------------|---------|
| `irishealth-community-*` | 1 (enabled) | Yes | MCP over HTTP, REST, web apps |
| `iris-community-*` | 0 (disabled) | No | ObjectScript/CLI only, no HTTP tools |

```bash
# ✅ For MCP over HTTP:
docker image load -i irishealth-community-2026.2.0AI.158.0-docker.tar.gz
docker run --name iris-ai-hub -p 1972:1972 -p 52773:52773 -d \
  irishealth/iris-community:2026.2.0AI.158.0

# ⚠️ Plain iris image has no web gateway — iris_execute HTTP path won't work
```

### Empty `Parameter APIKEY` auto-reads from OS environment

When `APIKEY` parameter is empty (the default), `%Init()` automatically resolves
`@{env.OPENAI_API_KEY}` (or the provider-appropriate env var).

```objectscript
Class MyAgent Extends %AI.Agent {
    Parameter PROVIDER = "openai";
    Parameter APIKEY;   // empty = reads OPENAI_API_KEY from OS env automatically
}

// Set in shell before starting IRIS:
// export OPENAI_API_KEY=sk-...

Set agent = ##class(MyAgent).%New()
$$$ThrowOnError(agent.%Init())   // picks up OPENAI_API_KEY from env
```

This is the recommended pattern for local dev. For production, use ConfigStore + Wallet.
