"""
032: E2E tests for compile-on-save hook script.
Uses iris-dev-iris container (the iris-dev project container).
Tests feed Claude plugin hook event JSON via subprocess stdin.
"""

import json
import os
import subprocess
import time
from pathlib import Path

import httpx
import pytest

CONTAINER_NAME = os.environ.get("IRIS_CONTAINER", "iris-dev-iris")
HOOK_SCRIPT = str(Path(__file__).parent.parent.parent / "scripts" / "compile-hook.sh")

POST_TOOL_USE_CLS = {
    "hook_event_name": "PostToolUse",
    "tool_name": "Write",
    "tool_input": {"file_path": "/workspace/Hook/TestError.cls"},
    "tool_result": {},
    "cwd": "/workspace",
}

POST_TOOL_USE_JSON = {
    "hook_event_name": "PostToolUse",
    "tool_name": "Write",
    "tool_input": {"file_path": "/workspace/config.json"},
    "tool_result": {},
    "cwd": "/workspace",
}

FILE_CHANGED_CLS = {
    "hook_event_name": "FileChanged",
    "file_path": "/workspace/Hook/TestError.cls",
}


def run_hook(event: dict, env_override: dict | None = None) -> tuple[str, int]:
    env = {**os.environ, **(env_override or {})}
    result = subprocess.run(
        ["bash", HOOK_SCRIPT],
        input=json.dumps(event),
        capture_output=True,
        text=True,
        env=env,
    )
    return result.stdout.strip(), result.returncode


@pytest.fixture(scope="session")
def iris_env():
    # Use env vars directly if set (iris-dev-iris is on a known port)
    if os.environ.get("IRIS_HOST") and os.environ.get("IRIS_WEB_PORT"):
        yield {
            "IRIS_HOST": os.environ["IRIS_HOST"],
            "IRIS_WEB_PORT": os.environ["IRIS_WEB_PORT"],
            "IRIS_NAMESPACE": os.environ.get("IRIS_NAMESPACE", "USER"),
            "IRIS_USERNAME": os.environ.get("IRIS_USERNAME", "_SYSTEM"),
            "IRIS_PASSWORD": os.environ.get("IRIS_PASSWORD", "SYS"),
        }
        return

    try:
        import docker as docker_sdk
        from iris_devtester import IRISContainer
    except ImportError:
        pytest.skip("iris_devtester or docker not installed")

    client = docker_sdk.from_env()
    try:
        client.containers.get(CONTAINER_NAME)
        container = IRISContainer.attach(CONTAINER_NAME)
    except docker_sdk.errors.NotFound:
        pytest.skip(
            f"Container '{CONTAINER_NAME}' not running and IRIS_HOST/IRIS_WEB_PORT not set."
        )

    host = container.get_container_host_ip()
    web_port = container.get_exposed_port(52773)
    try:
        username = container.get_username()
        password = container.get_password()
    except Exception:
        username, password = "_SYSTEM", "SYS"

    yield {
        "IRIS_HOST": host,
        "IRIS_WEB_PORT": str(web_port),
        "IRIS_NAMESPACE": "USER",
        "IRIS_USERNAME": username,
        "IRIS_PASSWORD": password,
    }


@pytest.fixture(scope="session")
def seed_error_class(iris_env):
    host = iris_env["IRIS_HOST"]
    port = iris_env["IRIS_WEB_PORT"]
    auth = (iris_env["IRIS_USERNAME"], iris_env["IRIS_PASSWORD"])
    base = f"http://{host}:{port}/api/atelier/v1/USER"

    lines = [
        "Class Hook.TestError Extends %RegisteredObject",
        "{",
        "Method DoSomething()",
        "{",
        "    Quit 1",
    ]
    resp = httpx.put(
        f"{base}/doc/Hook.TestError.cls",
        auth=auth,
        timeout=30,
        json={"enc": False, "content": lines},
    )
    assert resp.is_success or resp.status_code == 409, f"Seed failed: {resp.text}"
    yield "Hook.TestError.cls"


@pytest.fixture(scope="session")
def seed_clean_class(iris_env):
    host = iris_env["IRIS_HOST"]
    port = iris_env["IRIS_WEB_PORT"]
    auth = (iris_env["IRIS_USERNAME"], iris_env["IRIS_PASSWORD"])
    base = f"http://{host}:{port}/api/atelier/v1/USER"

    lines = [
        "Class Hook.TestClean Extends %RegisteredObject",
        "{",
        "Method DoSomething() As %Integer",
        "{",
        "    Quit 1",
        "}",
        "}",
    ]
    resp = httpx.put(
        f"{base}/doc/Hook.TestClean.cls",
        auth=auth,
        timeout=30,
        json={"enc": False, "content": lines},
    )
    assert resp.is_success or resp.status_code == 409, f"Seed failed: {resp.text}"
    yield "Hook.TestClean.cls"


class TestPostToolUseHook:
    def test_compile_cls_with_error(self, iris_env, seed_error_class):
        """T005: PostToolUse for .cls with syntax error — output contains error text."""
        event = {
            **POST_TOOL_USE_CLS,
            "tool_input": {"file_path": "/workspace/Hook/TestError.cls"},
        }
        output, _ = run_hook(event, env_override=iris_env)

        assert output != "", "Expected error output, got silent exit"
        assert "error" in output.lower() or "ERROR" in output, (
            f"Expected error text in output, got: {output}"
        )

    def test_compile_cls_clean(self, iris_env, seed_clean_class):
        """T006: PostToolUse for valid .cls — output contains 'OK'."""
        event = {
            **POST_TOOL_USE_CLS,
            "tool_input": {"file_path": "/workspace/Hook/TestClean.cls"},
        }
        output, _ = run_hook(event, env_override=iris_env)

        assert "OK" in output.upper() or "ok" in output.lower(), (
            f"Expected 'OK' in output, got: {output}"
        )

    def test_non_objectscript_silent(self):
        """T007: PostToolUse for .json file — silent exit, no output, no IRIS needed."""
        output, code = run_hook(POST_TOOL_USE_JSON)

        assert output == "", f"Expected silent exit for .json, got: {output}"
        assert code == 0

    def test_auto_compile_false_silent(self):
        """T008: IRIS_AUTO_COMPILE=false — silent exit even for .cls, no IRIS needed."""
        output, code = run_hook(
            POST_TOOL_USE_CLS,
            env_override={"IRIS_AUTO_COMPILE": "false"},
        )

        assert output == "", f"Expected silent exit when disabled, got: {output}"
        assert code == 0

    def test_iris_unavailable_message(self):
        """T009: No IRIS_HOST — clear message within 3.5 seconds."""
        env_no_iris = {k: v for k, v in os.environ.items() if k != "IRIS_HOST"}
        env_no_iris.pop("IRIS_HOST", None)
        env_no_iris["IRIS_HOST"] = ""

        start = time.monotonic()
        output, _ = run_hook(POST_TOOL_USE_CLS, env_override={"IRIS_HOST": ""})
        elapsed = time.monotonic() - start

        assert "not connected" in output.lower() or "IRIS_HOST" in output, (
            f"Expected 'not connected' message, got: {output}"
        )
        assert elapsed <= 3.5, f"Expected completion in ≤3.5s, took {elapsed:.1f}s"


class TestFileChangedHook:
    def test_file_changed_opt_in(self, iris_env, seed_clean_class):
        """T019: FileChanged with IRIS_COMPILE_ON_SAVE=true — compile runs."""
        event = {**FILE_CHANGED_CLS, "file_path": "/workspace/Hook/TestClean.cls"}
        env = {**iris_env, "IRIS_COMPILE_ON_SAVE": "true"}
        output, _ = run_hook(event, env_override=env)

        assert output != "", "Expected compile output, got silent exit"

    def test_file_changed_disabled_by_default(self):
        """T020: FileChanged without IRIS_COMPILE_ON_SAVE — silent exit."""
        env_without = {k: v for k, v in os.environ.items()}
        env_without.pop("IRIS_COMPILE_ON_SAVE", None)

        output, code = run_hook(FILE_CHANGED_CLS, env_override=env_without)

        assert output == "", f"Expected silent exit by default, got: {output}"
        assert code == 0
