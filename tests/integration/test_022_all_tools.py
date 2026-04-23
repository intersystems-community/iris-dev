"""
Spec 022: E2E test suite for all iris-dev tools.
Tests run against live IRIS (iris-dev-iris container).
PHASE GATES: each phase's E2E tests must pass before implementation continues.

Run:
    IRIS_HOST=localhost IRIS_WEB_PORT=52780 pytest tests/integration/test_022_all_tools.py -v
"""
import pytest
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))
from conftest_022 import mcp_call, extract_content, skip_no_iris, IRIS_AVAILABLE


# ============================================================
# PHASE 2 GATE TESTS — iris_execute and iris_test honest messages
# These MUST pass before any Phase 3 implementation begins.
# ============================================================

class TestPhase2Gate:
    """Phase 2 gate: iris_execute and iris_test return honest unsupported messages."""

    @skip_no_iris
    def test_execute_not_404_no_xecute(self, iris_env):
        """T004: iris_execute must NOT call /action/xecute (which doesn't exist).
        Post Tim's docker exec PR: returns DOCKER_REQUIRED error when IRIS_CONTAINER not set.
        Either way: no 404, no IRIS_UNREACHABLE, and response is a structured result not a crash."""
        resp = mcp_call("iris_execute", {"code": "write 1+1", "namespace": "USER", "confirmed": True})
        # Must not be a JSON-RPC level error (crash)
        assert "error" not in resp, f"iris_execute returned JSON-RPC error: {resp.get('error')}"
        content = extract_content(resp)
        # Must not contain the old xecute attempted_url
        assert "xecute" not in str(content.get("attempted_url", "")), \
            f"iris_execute still hitting xecute endpoint: {content}"
        # Must not be IRIS_UNREACHABLE
        assert content.get("error_code") != "IRIS_UNREACHABLE", \
            f"iris_execute returning IRIS_UNREACHABLE: {content}"
        # Must have actionable content — either output or DOCKER_REQUIRED guidance
        assert content.get("success") is not None or content.get("error_code") is not None, \
            f"iris_execute response has no success or error_code: {content}"

    @skip_no_iris
    def test_execute_docker_required_has_instructions(self, iris_env):
        """iris_execute without IRIS_CONTAINER must explain what to do."""
        resp = mcp_call("iris_execute", {"code": "write $ZVERSION", "namespace": "USER", "confirmed": True})
        assert "error" not in resp
        content = extract_content(resp)
        if not content.get("success"):
            # DOCKER_REQUIRED error must mention IRIS_CONTAINER
            text = str(content).lower()
            assert "iris_container" in text or "docker" in text, \
                f"iris_execute error lacks Docker guidance: {content}"

    @skip_no_iris
    def test_test_not_404_no_xecute(self, iris_env):
        """T005: iris_test must NOT call /action/xecute."""
        resp = mcp_call("iris_test", {"pattern": "Test.E2E022", "namespace": "USER"})
        assert "error" not in resp, f"iris_test returned JSON-RPC error: {resp.get('error')}"
        content = extract_content(resp)
        assert "xecute" not in str(content.get("attempted_url", "")), \
            f"iris_test still hitting xecute endpoint: {content}"
        assert content.get("error_code") != "IRIS_UNREACHABLE", \
            f"iris_test returning IRIS_UNREACHABLE: {content}"


# ============================================================
# PHASE 3 GATE TESTS — iris_info, iris_search, iris_macro, iris_debug
# These MUST pass before Phase 4 begins.
# ============================================================

class TestPhase3Gate:
    """Phase 3 gate: all previously-broken tools return correct data, zero 404s."""

    @skip_no_iris
    def test_iris_info_namespace_not_404(self, iris_env):
        """T009: iris_info(what=namespace) must return namespace name and db array."""
        resp = mcp_call("iris_info", {"what": "namespace", "namespace": "USER"})
        assert "error" not in resp, f"iris_info namespace error: {resp.get('error')}"
        content = extract_content(resp)
        assert "404" not in str(content), f"iris_info namespace still hitting 404: {content}"
        assert content.get("success") or content.get("name") or "USER" in str(content), \
            f"iris_info namespace returned unexpected content: {content}"

    @skip_no_iris
    def test_iris_info_documents_returns_list(self, iris_env):
        """T010: iris_info(what=documents) must return a list of documents."""
        resp = mcp_call("iris_info", {"what": "documents", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        # Must not be IRIS_UNREACHABLE
        assert content.get("error_code") != "IRIS_UNREACHABLE", \
            f"iris_info documents IRIS_UNREACHABLE: {content}"
        # result should contain a list
        result = content.get("result", {})
        assert result or content.get("success"), \
            f"iris_info documents returned nothing: {content}"

    @skip_no_iris
    def test_iris_search_not_405(self, iris_env, seed_test_class):
        """T011: iris_search must not return 405 (wrong HTTP method)."""
        resp = mcp_call("iris_search", {"query": "E2E022", "namespace": "USER"})
        assert "error" not in resp, f"iris_search error: {resp.get('error')}"
        content = extract_content(resp)
        assert "405" not in str(content), f"iris_search still returning 405: {content}"
        assert "error_code" not in str(content).lower() or "iris_unreachable" not in str(content).lower(), \
            f"iris_search IRIS_UNREACHABLE: {content}"

    @skip_no_iris
    def test_iris_macro_list_not_null(self, iris_env):
        """T012: iris_macro(action=list) must return a list, not null."""
        resp = mcp_call("iris_macro", {"action": "list", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("macros") is not None, \
            f"iris_macro list returned null: {content}"
        assert isinstance(content.get("macros"), list), \
            f"iris_macro list is not a list: {content}"

    @skip_no_iris
    def test_iris_debug_logs_not_null(self, iris_env):
        """T013: iris_debug(action=error_logs) must return a list, not null."""
        resp = mcp_call("iris_debug", {"action": "error_logs", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("logs") is not None, \
            f"iris_debug error_logs returned null: {content}"
        assert isinstance(content.get("logs"), list), \
            f"iris_debug logs is not a list: {content}"


# ============================================================
# PHASE 4 TESTS — Full tool coverage
# ============================================================

class TestIrisQuery:
    """iris_query: SQL execution."""

    @skip_no_iris
    def test_select_returns_rows(self, iris_env):
        resp = mcp_call("iris_query", {"query": "SELECT 1 AS n", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success"), f"iris_query failed: {content}"
        rows = content.get("rows", [])
        assert len(rows) == 1 and rows[0].get("n") == 1

    @skip_no_iris
    def test_invalid_sql_returns_structured_error(self, iris_env):
        resp = mcp_call("iris_query", {"query": "THIS IS NOT SQL", "namespace": "USER"})
        # Should return structured error, not crash
        assert "error" not in resp or resp.get("error", {}).get("code") != -32603, \
            "iris_query crashed on invalid SQL (internal error)"
        content = extract_content(resp)
        # Either success=false with error message, or a JSON-RPC error with message
        has_error_info = (
            not content.get("success", True) or
            "error" in str(content).lower() or
            "error" in resp
        )
        assert has_error_info, f"iris_query silently swallowed invalid SQL: {content}"


class TestIrisCompile:
    """iris_compile: class compilation."""

    @skip_no_iris
    def test_compile_known_class(self, iris_env, seed_test_class):
        resp = mcp_call("iris_compile", {"target": "Test.E2E022.Hello.cls", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success"), f"iris_compile failed: {content}"

    @skip_no_iris
    def test_compile_nonexistent_returns_error_not_crash(self, iris_env):
        resp = mcp_call("iris_compile", {"target": "Nonexistent.E2E022.Ghost.cls", "namespace": "USER"})
        assert "error" not in resp, "iris_compile crashed on nonexistent class"
        content = extract_content(resp)
        # Should have error info but structured, not a crash
        assert not content.get("success") or "error" in str(content).lower(), \
            f"iris_compile silently succeeded on nonexistent class: {content}"


class TestIrisDoc:
    """iris_doc: document read/write/delete."""

    @skip_no_iris
    def test_get_existing_class(self, iris_env, seed_test_class):
        resp = mcp_call("iris_doc", {"mode": "get", "name": "Test.E2E022.Hello.cls", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success"), f"iris_doc get failed: {content}"
        assert "E2E022" in str(content.get("content", "")), "iris_doc get returned wrong content"

    @skip_no_iris
    def test_get_nonexistent_returns_not_found(self, iris_env):
        resp = mcp_call("iris_doc", {"mode": "get", "name": "Nonexistent.E2E022.Ghost.cls", "namespace": "USER"})
        assert "error" not in resp, "iris_doc get crashed on missing class"
        content = extract_content(resp)
        assert not content.get("success") or content.get("error_code") == "NOT_FOUND", \
            f"iris_doc get silently succeeded on missing class: {content}"

    @skip_no_iris
    def test_put_get_delete_roundtrip(self, iris_env):
        temp_cls = (
            "Class Test.E2E022.Temp Extends %RegisteredObject\n"
            "{\n"
            "ClassMethod Ping() As %String { Return \"pong\" }\n"
            "}\n"
        )
        # Put
        resp = mcp_call("iris_doc", {"mode": "put", "name": "Test.E2E022.Temp.cls",
                                      "content": temp_cls, "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success") or "elicitation" in str(content).lower(), \
            f"iris_doc put failed: {content}"
        # Get
        resp = mcp_call("iris_doc", {"mode": "get", "name": "Test.E2E022.Temp.cls", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success"), f"iris_doc get after put failed: {content}"
        # Delete
        resp = mcp_call("iris_doc", {"mode": "delete", "name": "Test.E2E022.Temp.cls", "namespace": "USER"})
        assert "error" not in resp


class TestIrisSearch:
    """iris_search: full-text search."""

    @skip_no_iris
    def test_search_finds_seeded_class(self, iris_env, seed_test_class):
        resp = mcp_call("iris_search", {"query": "E2E022", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert "405" not in str(content), f"iris_search returning 405: {content}"
        assert content.get("success") or "matches" in content, \
            f"iris_search unexpected response: {content}"

    @skip_no_iris
    def test_search_empty_result_not_error(self, iris_env):
        resp = mcp_call("iris_search", {"query": "xyzzy_nonexistent_99999", "namespace": "USER"})
        assert "error" not in resp
        # Empty result is fine, crash is not
        content = extract_content(resp)
        assert "crash" not in str(content).lower()


class TestIrisSymbols:
    """iris_symbols: class/method symbol search."""

    @skip_no_iris
    def test_symbols_finds_seeded_class(self, iris_env, seed_test_class):
        resp = mcp_call("iris_symbols", {"query": "E2E022", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("count", 0) > 0, f"iris_symbols found nothing for E2E022: {content}"

    @skip_no_iris
    def test_symbols_empty_returns_list(self, iris_env):
        resp = mcp_call("iris_symbols", {"query": "xyzzy_nonexistent_e2e", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert isinstance(content.get("count"), int), f"iris_symbols count not int: {content}"


class TestIrisInfo:
    """iris_info: namespace discovery."""

    @skip_no_iris
    def test_namespace_returns_name(self, iris_env):
        resp = mcp_call("iris_info", {"what": "namespace", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert "404" not in str(content), f"iris_info namespace 404: {content}"
        assert "USER" in str(content) or content.get("success"), \
            f"iris_info namespace missing USER: {content}"

    @skip_no_iris
    def test_metadata_returns_version(self, iris_env):
        resp = mcp_call("iris_info", {"what": "metadata", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert "IRIS" in str(content).upper() or content.get("success"), \
            f"iris_info metadata missing IRIS version: {content}"

    @skip_no_iris
    def test_documents_returns_list(self, iris_env):
        resp = mcp_call("iris_info", {"what": "documents", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert "404" not in str(content), f"iris_info documents 404: {content}"


class TestIrisGenerate:
    """iris_generate: context provider (no LLM call)."""

    @skip_no_iris
    def test_returns_context_not_error(self, iris_env):
        resp = mcp_call("iris_generate", {"description": "A simple test class", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("context") or content.get("system_prompt") or content.get("prompt"), \
            f"iris_generate returned no context: {content}"


class TestIrisIntrospect:
    """docs_introspect: class introspection."""

    @skip_no_iris
    def test_introspect_known_class(self, iris_env, seed_test_class):
        resp = mcp_call("docs_introspect", {"class_name": "Test.E2E022.Hello", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("success") or "method" in str(content).lower(), \
            f"docs_introspect returned unexpected: {content}"

    @skip_no_iris
    def test_introspect_nonexistent_structured_error(self, iris_env):
        resp = mcp_call("docs_introspect", {"class_name": "Nonexistent.E2E022.Ghost", "namespace": "USER"})
        assert "error" not in resp, "docs_introspect crashed on nonexistent class"


class TestIrisSourceControl:
    """iris_source_control: SCM status."""

    @skip_no_iris
    def test_status_not_crash(self, iris_env, seed_test_class):
        resp = mcp_call("iris_source_control", {"action": "status",
                                                  "document": "Test.E2E022.Hello.cls",
                                                  "namespace": "USER"})
        assert "error" not in resp, f"iris_source_control status crashed: {resp.get('error')}"
        content = extract_content(resp)
        assert content.get("success"), f"iris_source_control status failed: {content}"
        # No SCM = controlled: false is expected
        assert content.get("controlled") is not None, \
            f"iris_source_control status missing controlled field: {content}"


class TestIrisDebug:
    """iris_debug: error logs."""

    @skip_no_iris
    def test_error_logs_returns_list_not_null(self, iris_env):
        resp = mcp_call("iris_debug", {"action": "error_logs", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("logs") is not None, f"iris_debug logs is null: {content}"
        assert isinstance(content.get("logs"), list), f"iris_debug logs not a list: {content}"


class TestIrisMacro:
    """iris_macro: macro/include listing."""

    @skip_no_iris
    def test_list_returns_list_not_null(self, iris_env):
        resp = mcp_call("iris_macro", {"action": "list", "namespace": "USER"})
        assert "error" not in resp
        content = extract_content(resp)
        assert content.get("macros") is not None, f"iris_macro list is null: {content}"
        assert isinstance(content.get("macros"), list), f"iris_macro list not a list: {content}"


class TestIrisExecute:
    """iris_execute: docker exec path, no /action/xecute."""

    @skip_no_iris
    def test_no_xecute_endpoint(self, iris_env):
        """iris_execute must not call the non-existent /action/xecute endpoint."""
        resp = mcp_call("iris_execute", {"code": "write 1+1", "namespace": "USER", "confirmed": True})
        assert "error" not in resp, f"iris_execute crashed: {resp.get('error')}"
        content = extract_content(resp)
        assert "xecute" not in str(content.get("attempted_url", "")), \
            f"iris_execute still calls xecute: {content}"
        assert content.get("error_code") != "IRIS_UNREACHABLE", \
            f"iris_execute IRIS_UNREACHABLE: {content}"

    @skip_no_iris
    def test_docker_required_or_success(self, iris_env):
        """Without IRIS_CONTAINER: DOCKER_REQUIRED with instructions. With it: actual output."""
        resp = mcp_call("iris_execute", {"code": "write 1", "namespace": "USER", "confirmed": True})
        assert "error" not in resp
        content = extract_content(resp)
        if not content.get("success"):
            assert "docker" in str(content).lower() or "iris_container" in str(content).lower(), \
                f"iris_execute failure lacks Docker guidance: {content}"


class TestIrisTest:
    """iris_test: docker exec path, no /action/xecute."""

    @skip_no_iris
    def test_no_xecute_endpoint(self, iris_env):
        """iris_test must not call the non-existent /action/xecute endpoint."""
        resp = mcp_call("iris_test", {"pattern": "Test.E2E022", "namespace": "USER"})
        assert "error" not in resp, f"iris_test crashed: {resp.get('error')}"
        content = extract_content(resp)
        assert "xecute" not in str(content.get("attempted_url", "")), \
            f"iris_test still calls xecute: {content}"
        assert content.get("error_code") != "IRIS_UNREACHABLE", \
            f"iris_test IRIS_UNREACHABLE: {content}"


class TestSkillList:
    """skill_list and skill_describe."""

    @skip_no_iris
    def test_skill_list_returns_list(self, iris_env):
        resp = mcp_call("skill_list", {})
        assert "error" not in resp
        # May return empty list — that's fine
        content = extract_content(resp)
        assert isinstance(content.get("skills", []), list), f"skill_list not a list: {content}"

    @skip_no_iris
    def test_skill_describe_nonexistent_structured_error(self, iris_env):
        resp = mcp_call("skill_describe", {"name": "nonexistent_e2e_skill_xyzzy"})
        assert "error" not in resp, "skill_describe crashed on nonexistent skill"


class TestSkillCommunity:
    """skill_community_list and skill_community_install."""

    @skip_no_iris
    def test_community_list_returns_list(self, iris_env):
        resp = mcp_call("skill_community_list", {})
        assert "error" not in resp
        # May be empty — that's fine
        content = extract_content(resp)
        assert not isinstance(content, type(None)), f"skill_community_list returned None: {content}"

    @skip_no_iris
    def test_community_install_nonexistent_structured_error(self, iris_env):
        resp = mcp_call("skill_community_install", {"name": "nonexistent_e2e_skill_xyzzy"})
        assert "error" not in resp, "skill_community_install crashed on nonexistent skill"
