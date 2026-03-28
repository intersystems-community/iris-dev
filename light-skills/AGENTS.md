# AGENTS for ObjectScript AI Coding

Drop this file in your repo root (or `.claude/AGENTS.md`) so AI coding agents understand
ObjectScript semantics before writing a single line of code.

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

**Critical**: Always compile after writing ObjectScript. Feed compiler errors back and ask for fixes. Never assume code is correct without compilation.

### Via IRIS terminal session
```bash
# Compile a single class (file must be visible from IRIS working dir)
iris session IRIS -U USER "Do \$System.OBJ.Load(\"MyPackage/MyClass.cls\",\"ck\")"

# Compile by class name (already exists in IRIS)
iris session IRIS -U USER "Do \$System.OBJ.Compile(\"MyPackage.MyClass\",\"ck\")"

# Run %UnitTest tests
iris session IRIS -U USER "Do ##class(%UnitTest.Manager).RunTest(\"MyPackage.Tests\",,\"/nodelete\")"
```

### Via Atelier REST API (see introspect.md and compile.md skills)
```bash
# Upload and compile in one shot
curl -s -X PUT "http://localhost:52773/api/atelier/v1/USER/doc/MyPackage.MyClass.cls" \
  -u _SYSTEM:SYS \
  -H "Content-Type: application/json" \
  -d '{"enc": false, "content": ["Class MyPackage.MyClass ..."]}'
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

## 4. Namespace & Environment Awareness

- **Always ask which namespace** before writing code that touches globals or calls existing classes. IRIS can have many namespaces (`USER`, `HSCUSTOM`, `HSLIB`, application-specific) with different class libraries.
- **`%SYS` is privileged** — system-level operations (user management, license info) require `%SYS`. Don't put application code there.
- **IRIS web port ≠ superserver port** — The Atelier/REST web server listens on `52773` by default (or a Docker-mapped port). The superserver (JDBC/DBAPI) is on `1972`. These are different.
- **Check namespace before class search** — `Do $System.Status.DisplayError(##class(%Dictionary.ClassDefinition).%OpenId("My.Class"))` returning an error likely means you're in the wrong namespace, not that the class doesn't exist.

---

## 5. Using AI Skills (no MCP server required)

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
