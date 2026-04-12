# AGENTS for ObjectScript AI Coding

Drop this file in your repo root (or `.claude/AGENTS.md`) so AI coding agents understand
ObjectScript semantics before writing a single line of code.

> **Primary workflow**: Most ISC SEs and developers use VS Code with the [ObjectScript MCP extension](https://github.com/intersystems-community/vscode-objectscript-mcp) — it wires `iris_compile`, `iris_test`, `docs_introspect`, and `interop_*` tools directly into Copilot agent mode using your existing `objectscript.conn` settings. This AGENTS.md covers the ObjectScript rules those tools operate under.

> **Benchmark result**: Claude Sonnet 4.6 with this file scores **86% on a 22-task ObjectScript repair suite** (+14% lift over no context). The `objectscript-review` skill below raises that to **100%** (+29% lift).

---

## HARD GATE — Run before showing any ObjectScript code

Load `skills/objectscript-review/SKILL.md` if available. Otherwise apply this checklist mentally:

- [ ] No `Quit <value>` inside For/While loops — use `Return <value>`
- [ ] Postfix condition has no spaces: `Quit:key=""` not `Quit:key = ""`
- [ ] `$IsObject(obj)` checked after every `%OpenId` before accessing properties
- [ ] SQL table name: `MF.Catalog.Product` → `MF_Catalog.Product` (last dot = schema separator)
- [ ] `SQLCODE = 0` is success (falsy) — check `SQLCODE = 0`, not just `SQLCODE`
- [ ] HTML escaping: `&` FIRST, then `<`, then `>`
- [ ] Arithmetic is left-to-right: use `1.8` not `9/5`, parenthesize everything
- [ ] `$ListBuild()` creates a list of length 1, not 0 — use `""` for empty list
- [ ] `%Status`: use `$$$ISERR(sc)` — never return `$$$OK` after catching an error
- [ ] Transactions: `If $TLevel > 0 { TRollback }` — never `Return` inside `TStart` without rollback

---

## 1. ObjectScript Language Rules (LLM Gotchas)

### Control Flow
1. **`Quit` vs `Return`** — `Quit <value>` is **illegal** inside `TRY/CATCH` or loops. Use `Return <value>` to exit a method with a value; use bare `Quit` only to exit the current loop or `FOR` block. When in doubt, use `Return`.
2. **No operator precedence** — ObjectScript evaluates **strictly left-to-right**. `3+3*2 = 12`, not `9`. Always use parentheses for compound expressions.
3. **No `finally`, single `Catch`** — `TRY/CATCH` has exactly one `Catch` block and no `finally`. Cleanup goes after the block. Differentiate exception types with `ex.%IsA("...")`.

### Methods & Variables
4. **Intra-class method calls require `..`** — Inside a method, call `Do ..MyMethod()` not `Do MyMethod()`. `##class(Same.Class).MyMethod()` also works but `..` is idiomatic for same-class calls.
5. **`NEW` is illegal inside methods** — Never use `New varname` inside a method body; method/procedure blocks are already isolated in scope.
6. **Instance variables** — `i%PropertyName` accesses the raw slot directly; `..PropertyName` goes through the accessor. Prefer `..PropertyName` unless you have a specific reason not to.

### Error Handling
7. **`%Status` return convention** — Methods that can fail return `%Status`. Check with `$$$ISOK(sc)` / `$$$ISERR(sc)`. Return `$$$OK` on success. Never compare `If sc=0`; always use the macros.
8. **Throwing and catching** — Use `$$$ThrowOnError(sc)` to throw on failure. Never `Throw sc` directly — `THROW` expects a `%Exception.AbstractException`. Correct pattern:
   ```objectscript
   Try {
       $$$ThrowOnError(..DoSomething())
   } Catch ex {
       Set sc = ex.AsStatus()
   }
   ```
9. **Transaction discipline** — Always check `$TLEVEL` before rolling back. Standard pattern:
   ```objectscript
   TStart
   Try {
       // work
       TCommit
   } Catch ex {
       If $TLevel > 0 TRollback
       Set sc = ex.AsStatus()
   }
   ```

### Types & Formats
10. **`%TimeStamp` format** — `%TimeStamp` uses `YYYY-MM-DD HH:MM:SS` (a space, not `T`). **Not** ISO 8601 with `T`. This is the most common AI mistake. Always use space-separated format for any IRIS date/time literal.
11. **String concatenation** — Use `_` to concatenate strings: `"Hello" _ " " _ name`. There is no `+` for strings.
12. **Globals vs locals** — `^GlobalName` is database-persistent and shared across processes. Local variables (`var`) are process-scoped and temporary. Never use globals as temporary variables.
13. **`$LISTNEXT` for list iteration** — To iterate a `%List`, use `$LISTNEXT(list, ptr, value)` with `Set ptr=0` before the loop. Do not use `FOR i=1:1:$LISTLENGTH(list)` — it is slower and error-prone for embedded lists.

---

## 2. Compile & Test Loop

**If the objectscript MCP server is connected (check `/mcp`), always use MCP tools first. Fall back to bash only if the MCP is unavailable.**

### Via MCP tools (preferred)
```
# Find classes by name pattern — live IRIS namespace
iris_symbols(query="MyPackage.*")
iris_symbols(query="%ASQ*")          # system classes
iris_symbols_local()                 # parse .cls files on disk, no IRIS needed

# Read a full class definition from IRIS (methods, parameters, inheritance)
docs_introspect(class_name="MyPackage.MyClass")
docs_introspect(class_name="%ASQ.Engine")   # works on system classes too

# Compile a .cls file
iris_compile(target="MyPackage/MyClass.cls", namespace="USER")

# Run %UnitTest tests
iris_test(pattern="MyPackage.Tests.*")

# If IRIS is unreachable — list containers and pick the right one
iris_list_containers()
iris_select_container(name="arno_iris_test")   # reconnects without restart
```

**Do NOT use `docker exec` / `docker cp` / `iris session` bash commands when the MCP is connected.** The MCP handles container targeting automatically after `iris_select_container`.

### Reading class source from IRIS (INT / system classes)
To read the source of any class — including system classes like `%ASQ.Engine` that have no `.cls` on disk:
```
# Option 1 (preferred): docs_introspect — returns parsed method signatures
docs_introspect(class_name="%ASQ.Engine")

# Option 2: export via OBJ then read — use when you need full source with macros
# Run in bash:
docker exec <container> iris session IRIS -U USER \
  "set sc = \$system.OBJ.ExportUDL(\"%ASQ.Engine.cls\",\"/tmp/ASQEngine.cls\") halt"
docker cp <container>:/tmp/ASQEngine.cls /tmp/ASQEngine.cls
# Then use Read tool on /tmp/ASQEngine.cls
```

### Via bash (fallback only — when MCP is unavailable)
```bash
# Compile a single class
iris session IRIS -U USER "Do \$System.OBJ.Load(\"MyPackage/MyClass.cls\",\"ck\")"

# Run %UnitTest tests
iris session IRIS -U USER "Do ##class(%UnitTest.Manager).RunTest(\"MyPackage.Tests\",,\"/nodelete\")"
```

### Reading compile errors
IRIS compiler errors look like:
```
ERROR #5659: Method 'Foo' in class 'My.Class' has a 'Return' that does not match the return type
ERROR #5002: ObjectScript error in method 'Bar' in class 'My.Class'  <UNDEFINED>var+3^My.Class.1
```
- The `+3^My.Class.1` suffix means line 3 of the compiled `.INT` routine — map back to your `.cls` source.
- `<UNDEFINED>` means a variable was used before being set.
- `<NOLINE>` at compile time usually means a syntax error above the reported line.

---

## 3. Class Structure Templates

### Standard class with %Status error handling
```objectscript
Class MyPackage.MyClass Extends %RegisteredObject
{

/// Brief description of what this method does.
ClassMethod MyMethod(pArg As %String) As %Status
{
    Set sc = $$$OK
    Try {
        // implementation
        $$$ThrowOnError(..HelperMethod(pArg))
    } Catch ex {
        Set sc = ex.AsStatus()
    }
    Return sc
}

/// Returns a value or throws.
ClassMethod GetValue(pKey As %String) As %String
{
    Set val = $Get(^MyGlobal(pKey))
    If val = "" $$$ThrowStatus($$$ERROR($$$GeneralError, "Key not found: " _ pKey))
    Return val
}

}
```

### Persistent class
```objectscript
Class MyPackage.MyRecord Extends %Persistent
{

Property Name As %String(MAXLEN = 255);
Property CreatedAt As %TimeStamp;  // stored as YYYY-MM-DD HH:MM:SS

Index NameIdx On Name;

ClassMethod FindByName(pName As %String) As MyPackage.MyRecord
{
    Return ##class(MyPackage.MyRecord).NameIndexOpen(pName)
}

}
```

### %UnitTest test class
```objectscript
Class MyPackage.Tests.MyClassTest Extends %UnitTest.TestCase
{

Method TestBasicCase()
{
    Set result = ##class(MyPackage.MyClass).MyMethod("input")
    Do $$$AssertStatusOK(result)
}

Method TestEdgeCase()
{
    // Test that invalid input returns an error status
    Set result = ##class(MyPackage.MyClass).MyMethod("")
    Do $$$AssertStatusNotOK(result)
}

}
```

---

## 4. Legacy .MAC Routines

**Most IRIS codebases — especially CHUI apps, integrations, and anything pre-2000 — are `.MAC` routines, not classes.** AI models default to class syntax. If you're working with `.MAC`, tell your agent explicitly and use these rules.

### Structure

`.MAC` routines use label-based structure, not class methods:

```objectscript
MYROUTINE
    ; Entry point — no parentheses, no braces
    Set x = 1
    Do HELPER
    Quit

HELPER
    ; Subroutine — called with DO HELPER or DO HELPER^MYROUTINE
    Write "hello", !
    Quit

CALC(a, b)
    ; Extrinsic function — called with $$CALC(1,2) or $$CALC^MYROUTINE(1,2)
    Quit a + b
```

### `#include` vs `Include` — different syntax than classes

```objectscript
; .MAC uses preprocessor directive — NOT the class keyword
#include %occStatus
#include myMacros

; In a class you'd write:
; Include %occStatus   ← class keyword, no #
; But in .MAC it MUST be:
; #include %occStatus  ← preprocessor directive, with #
```

### Calling conventions

```objectscript
; Call a subroutine in same routine:
Do LABEL

; Call a subroutine in another routine:
Do LABEL^OTHERROUTINE

; Call an extrinsic function (returns a value):
Set result = $$LABEL
Set result = $$LABEL^OTHERROUTINE(arg1, arg2)

; WRONG in .MAC — no class method syntax:
Set result = ..MyMethod()          ; ERROR — no object context in .MAC
Set result = ##class(X).Method()   ; Valid but uncommon in legacy .MAC
```

### Error handling — `$ZTRAP` not `Try/Catch`

Legacy `.MAC` uses `$ZTRAP` label-based error handling, not `Try/Catch`:

```objectscript
MYROUTINE
    Set $ZTRAP = "ERRHANDLER"
    ; ... code that might error ...
    Set $ZTRAP = ""
    Quit

ERRHANDLER
    Set $ZTRAP = ""        ; Clear trap to avoid recursion
    Write "Error: ", $ZE, !
    Quit
```

Modern `.MAC` can use `Try/Catch` — prefer it for new code even in `.MAC` files. Do not introduce `$ZTRAP` in new code.

### Variable scope — routines have no automatic isolation

```objectscript
; WRONG: variable bleeds across DO calls unless you NEW it
CALLER
    Set x = "original"
    Do CALLEE
    Write x, !    ; Might print "modified" — x was changed in CALLEE!

CALLEE
    Set x = "modified"
    Quit

; CORRECT: NEW isolates the variable
CALLEE
    New x
    Set x = "modified"
    Quit
```

### Key differences from classes

| .MAC routines | ObjectScript classes |
|---------------|---------------------|
| `#include file.inc` | `Include ClassName` |
| `Do LABEL^ROUTINE` | `Do ##class(X).Method()` |
| `$$FUNC^ROUTINE(args)` | `##class(X).Method(args)` |
| `$ZTRAP = "LABEL"` | `Try { } Catch e { }` |
| `New var` for scope | Method variables auto-scoped |
| No `..` for self-reference | `..Property`, `..Method()` |
| Globals for shared state | Properties for instance state |

---

## 5. Namespace & Environment Awareness

- **Always ask which namespace**
- **`%SYS` is privileged** — system-level operations (user management, license info) require `%SYS`. Don't put application code there.
- **IRIS web port ≠ superserver port** — The Atelier/REST web server listens on `52773` by default (or a Docker-mapped port). The superserver (JDBC/DBAPI) is on `1972`. These are different.
- **Check namespace before class search** — `Do $System.Status.DisplayError(##class(%Dictionary.ClassDefinition).%OpenId("My.Class"))` returning an error likely means you're in the wrong namespace, not that the class doesn't exist.

---

## 6. Using AI Skills (no MCP server required)

The `light-skills/` directory contains two standalone skills you can use with Claude Code,
opencode, or any agent that supports markdown skill files:

| Skill | What it does |
|---|---|
| `introspect.md` | Fetches a class definition from IRIS via Atelier REST — gives the AI full method signatures, parameters, and return types for any class in your IRIS instance |
| `compile.md` | Compiles a class via Atelier REST and returns structured error output for the AI to fix |

Copy them to `.claude/skills/` or `.opencode/skills/` in your repo. Then invoke:
- `/introspect MyPackage.MyClass` — before editing any class you haven't written
- `/compile MyPackage.MyClass` — after every edit, before declaring done

**These skills require only `curl` and a running IRIS web server** — no Python, no pip installs.

Set these env vars (or substitute directly):
```bash
export IRIS_HOST=localhost
export IRIS_WEB_PORT=52773
export IRIS_USER=_SYSTEM
export IRIS_PASS=SYS
export IRIS_NS=USER
```
