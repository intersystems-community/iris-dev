# Contract — Task YAML Schema

## Required Fields

| Field | Type | Description |
|-------|------|-------------|
| id | string | Unique ID, pattern `[A-Z]+-[0-9]+` |
| category | enum | GEN, MOD, DBG, SCM, LEG |
| description | string | Prompt sent verbatim to the agent |
| expected_behavior | string | What correct output looks like (for LLM judge) |
| path | enum | A, B, or both |

## Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| fixtures | list | IRIS state to create before task |
| tags | list | e.g. [basic, no-context, legacy] |
| timeout_s | int | Per-task timeout, default 120 |
| max_tool_calls | int | Score=2 threshold, default 2 |

## Fixture Types

```yaml
fixtures:
  - type: cls          # Write a .cls file to BENCHMARK namespace
    name: Bench.Helper
    content: |
      Class Bench.Helper { ... }

  - type: global       # Set a global value
    name: ^BenchData
    subscript: "key"
    value: "value"

  - type: routine      # Write a .mac routine
    name: BenchMac
    content: |
      ROUTINE BenchMac
      ...
```

## Example — GEN-01

```yaml
id: GEN-01
category: GEN
path: both
description: "Write an ObjectScript class called Bench.Greeter with a ClassMethod Hello() that returns the string 'Hello World'."
expected_behavior: "Class Bench.Greeter exists in BENCHMARK namespace, compiles without errors, and ##class(Bench.Greeter).Hello() returns 'Hello World'."
tags: [basic, no-context]
```

## Example — LEG-01 (Steve P scenario)

```yaml
id: LEG-01
category: LEG
path: both
description: "Write a MAC routine called BenchLeg that stores a patient name in the global ^BenchPatients with the patient ID as subscript, and retrieves it. No classes — globals and routines only."
expected_behavior: "Routine BenchLeg exists, compiles, and when called as do Store^BenchLeg(1,'Smith') followed by do Get^BenchLeg(1) the output is 'Smith'."
fixtures: []
tags: [legacy, mac, globals]
```
