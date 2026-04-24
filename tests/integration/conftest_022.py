"""conftest for spec-022 E2E tests — MCP call helper, seed fixtures, iris_env."""
#![allow(dead_code)]
import json
import os
import subprocess
import time
import pytest
import httpx

IRIS_HOST = os.environ.get("IRIS_HOST", "")
IRIS_WEB_PORT = os.environ.get("IRIS_WEB_PORT", "52780")
IRIS_USERNAME = os.environ.get("IRIS_USERNAME", "_SYSTEM")
IRIS_PASSWORD = os.environ.get("IRIS_PASSWORD", "SYS")
IRIS_NAMESPACE = os.environ.get("IRIS_NAMESPACE", "USER")

IRIS_AVAILABLE = bool(IRIS_HOST)

skip_no_iris = pytest.mark.skipif(
    not IRIS_AVAILABLE,
    reason="IRIS_HOST not set — skipping live IRIS tests"
)


def iris_base_url() -> str:
    return f"http://{IRIS_HOST}:{IRIS_WEB_PORT}/api/atelier"


def iris_auth():
    return (IRIS_USERNAME, IRIS_PASSWORD)


def mcp_call(tool: str, args: dict, timeout: float = 8.0) -> dict:
    """Call one iris-dev MCP tool via stdio. Returns the parsed result object."""
    env = {
        **os.environ,
        "IRIS_HOST": IRIS_HOST,
        "IRIS_WEB_PORT": IRIS_WEB_PORT,
        "IRIS_USERNAME": IRIS_USERNAME,
        "IRIS_PASSWORD": IRIS_PASSWORD,
        "IRIS_NAMESPACE": IRIS_NAMESPACE,
    }
    iris_dev = _find_iris_dev()
    proc = subprocess.Popen(
        [iris_dev, "mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=env,
    )
    try:
        msgs = [
            json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                        "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                                   "clientInfo": {"name": "test-022", "version": "1"}}}) + "\n",
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized",
                        "params": {}}) + "\n",
            json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                        "params": {"name": tool, "arguments": args}}) + "\n",
        ]
        for i, msg in enumerate(msgs):
            proc.stdin.write(msg.encode())
            proc.stdin.flush()
            time.sleep(0.2)
        time.sleep(timeout - 0.6)
        proc.stdin.close()
        out = proc.stdout.read().decode(errors="replace")
    finally:
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()

    for line in out.splitlines():
        try:
            obj = json.loads(line)
            if obj.get("id") == 2:
                return obj
        except json.JSONDecodeError:
            pass
    raise RuntimeError(f"No response for tool={tool} in output: {out[:300]}")


def extract_content(mcp_response: dict) -> dict:
    """Parse the nested content JSON from an MCP tool response."""
    result = mcp_response.get("result", {})
    content = result.get("content", [{}])
    if content:
        try:
            return json.loads(content[0].get("text", "{}"))
        except (json.JSONDecodeError, TypeError):
            return {"raw": content[0].get("text", "")}
    return {}


def _find_iris_dev() -> str:
    import shutil
    binary = shutil.which("iris-dev")
    if binary:
        return binary
    # Fall back to built binary in repo — check both release and debug builds
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))
    for build_type in ("release", "debug"):
        candidate = os.path.join(repo_root, "target", build_type, "iris-dev")
        if os.path.isfile(candidate):
            return candidate
    raise FileNotFoundError("iris-dev binary not found on PATH or in target/release/ or target/debug/")


@pytest.fixture(scope="session")
def iris_env():
    """Session-scoped IRIS connection info. Skips if IRIS_HOST not set."""
    if not IRIS_AVAILABLE:
        pytest.skip("IRIS_HOST not set")
    return {
        "host": IRIS_HOST,
        "port": IRIS_WEB_PORT,
        "username": IRIS_USERNAME,
        "password": IRIS_PASSWORD,
        "namespace": IRIS_NAMESPACE,
        "base_url": iris_base_url(),
        "auth": iris_auth(),
    }


@pytest.fixture(scope="session")
def seed_test_class(iris_env):
    """Write and compile Test.E2E022.Hello into USER namespace. Clean up after session."""
    base = iris_env["base_url"]
    auth = iris_env["auth"]
    ns = iris_env["namespace"]
    cls_content = (
        "Class Test.E2E022.Hello Extends %RegisteredObject\n"
        "{\n"
        "ClassMethod Greet() As %String\n"
        "{\n"
        "    Return \"Hello from E2E022\"\n"
        "}\n"
        "}\n"
    )
    lines = cls_content.splitlines()
    # Write
    resp = httpx.put(
        f"{base}/v8/{ns}/doc/Test.E2E022.Hello.cls",
        auth=auth,
        json={"enc": False, "content": lines},
        timeout=15,
    )
    assert resp.status_code in (200, 201, 409), f"Seed PUT failed: {resp.status_code} {resp.text[:200]}"
    # Compile
    resp = httpx.post(
        f"{base}/v1/{ns}/action/compile",
        auth=auth,
        json=["Test.E2E022.Hello.cls"],
        timeout=15,
    )
    assert resp.status_code == 200, f"Seed compile failed: {resp.status_code} {resp.text[:200]}"
    body = resp.json()
    errors = [e for e in body.get("result", {}).get("console", []) if "ERROR" in e.upper()]
    assert not errors, f"Seed compile errors: {errors}"

    yield "Test.E2E022.Hello.cls"

    # Cleanup
    httpx.delete(f"{base}/v8/{ns}/doc/Test.E2E022.Hello.cls", auth=auth, timeout=10)
