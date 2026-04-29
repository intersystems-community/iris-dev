"""BENCHMARK namespace setup and teardown via iris_execute."""
import os
import subprocess
import json
import time


def _mcp_call(tool: str, args: dict) -> dict:
    iris_host = os.environ.get("IRIS_HOST", "localhost")
    iris_port = os.environ.get("IRIS_WEB_PORT", "52780")
    iris_user = os.environ.get("IRIS_USERNAME", "_SYSTEM")
    iris_pass = os.environ.get("IRIS_PASSWORD", "SYS")

    msgs = [
        '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"benchmark","version":"1"}}}',
        '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}',
        json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": tool, "arguments": args}}),
    ]

    env = os.environ.copy()
    env.update({
        "IRIS_HOST": iris_host,
        "IRIS_WEB_PORT": iris_port,
        "IRIS_USERNAME": iris_user,
        "IRIS_PASSWORD": iris_pass,
    })

    proc = subprocess.Popen(
        ["iris-dev", "mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=env,
    )

    # send with small delays so server processes each message
    for i, msg in enumerate(msgs):
        proc.stdin.write((msg + "\n").encode())
        proc.stdin.flush()
        time.sleep(0.2)

    time.sleep(2)
    proc.stdin.close()
    out = proc.stdout.read().decode(errors="replace")
    proc.wait()

    for line in out.splitlines():
        try:
            obj = json.loads(line)
            if obj.get("id") == 2:
                return obj
        except json.JSONDecodeError:
            pass
    return {}


def reset_benchmark_namespace():
    """Drop and recreate the BENCHMARK namespace to eliminate carry-over between conditions.

    Called before each condition's 15-task run (FR-001b).
    """
    # Drop: kill all globals and delete the namespace
    drop_code = (
        'set sc=##class(%SYS.Namespace).Delete("BENCHMARK")'
        ' if $system.Status.IsError(sc) && $system.Status.GetErrorText(sc) \'[ "not exist" {'
        '  write "ERROR:"_$system.Status.GetErrorText(sc)'
        ' } else { write "DROPPED" }'
    )
    resp = _mcp_call("iris_execute", {"code": drop_code, "namespace": "%SYS", "confirmed": True})
    content = resp.get("result", {}).get("content", [{}])[0].get("text", "")
    if "ERROR" in content:
        raise RuntimeError(f"Failed to drop BENCHMARK namespace: {content}")

    # Recreate
    create_code = (
        'set sc=##class(%SYS.Namespace).Create("BENCHMARK")'
        ' if $system.Status.IsError(sc) { write "ERROR:"_$system.Status.GetErrorText(sc) }'
        ' else { write "CREATED" }'
    )
    resp = _mcp_call("iris_execute", {"code": create_code, "namespace": "%SYS", "confirmed": True})
    content = resp.get("result", {}).get("content", [{}])[0].get("text", "")
    if "ERROR" in content:
        raise RuntimeError(f"Failed to create BENCHMARK namespace: {content}")


def ensure_benchmark_namespace():
    """Create BENCHMARK namespace if it does not exist."""
    code = (
        'if \'##class(%SYS.Namespace).Exists("BENCHMARK") {'
        ' set sc=##class(%SYS.Namespace).Create("BENCHMARK")'
        ' if $system.Status.IsError(sc) { write "ERROR:"_$system.Status.GetErrorText(sc) }'
        ' else { write "CREATED" }'
        '} else { write "EXISTS" }'
    )
    resp = _mcp_call("iris_execute", {"code": code, "namespace": "%SYS", "confirmed": True})
    content = resp.get("result", {}).get("content", [{}])[0].get("text", "")
    if "ERROR" in content:
        raise RuntimeError(f"Failed to create BENCHMARK namespace: {content}")


def wipe_benchmark_namespace():
    """Kill all globals in BENCHMARK namespace between tasks."""
    code = "kill @(\"^\")"
    _mcp_call("iris_execute", {"code": code, "namespace": "BENCHMARK", "confirmed": True})
