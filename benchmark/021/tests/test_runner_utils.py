"""Unit tests for benchmark runner utilities: namespace reset and wrong_tool_count tracking.

T008: reset_benchmark_namespace() — written FIRST, must FAIL before implementation.
T009: wrong_tool_count tracking — written FIRST, must FAIL before implementation.
"""
import pytest
from unittest.mock import patch, MagicMock, call


# ── T008: reset_benchmark_namespace ──────────────────────────────────────────

class TestResetBenchmarkNamespace:
    """Tests for the full drop+recreate namespace reset (FR-001b)."""

    def test_reset_calls_drop_then_create(self):
        """reset_benchmark_namespace() must drop then recreate BENCHMARK namespace."""
        from namespace import reset_benchmark_namespace  # noqa: F401 — will fail until T005 implemented

        calls = []
        def fake_mcp_call(tool, args):
            calls.append((tool, args.get("code", "")))
            return {"result": {"content": [{"text": "OK"}]}}

        with patch("namespace._mcp_call", fake_mcp_call):
            reset_benchmark_namespace()

        assert len(calls) == 2, f"Expected 2 MCP calls, got {len(calls)}: {calls}"
        # First call should reference DROP or KILL or delete namespace
        assert any(keyword in calls[0][1].upper() for keyword in ("DROP", "DELETE", "KILL", "REMOVE")), \
            f"First call should drop namespace, got: {calls[0]}"
        # Second call should CREATE
        assert "CREATE" in calls[1][1].upper(), \
            f"Second call should create namespace, got: {calls[1]}"

    def test_reset_uses_benchmark_namespace_name(self):
        """reset_benchmark_namespace() must reference BENCHMARK namespace."""
        from namespace import reset_benchmark_namespace  # noqa

        captured = []
        def fake_mcp_call(tool, args):
            captured.append(args)
            return {"result": {"content": [{"text": "OK"}]}}

        with patch("namespace._mcp_call", fake_mcp_call):
            reset_benchmark_namespace()

        all_args = str(captured)
        assert "BENCHMARK" in all_args, \
            f"BENCHMARK namespace name must appear in MCP calls, got: {all_args}"

    def test_reset_raises_on_error(self):
        """reset_benchmark_namespace() must raise RuntimeError if MCP returns error."""
        from namespace import reset_benchmark_namespace  # noqa

        def fake_mcp_call(tool, args):
            return {"result": {"content": [{"text": "ERROR: permission denied"}]}}

        with patch("namespace._mcp_call", fake_mcp_call):
            with pytest.raises(RuntimeError, match="(?i)error|fail|permission"):
                reset_benchmark_namespace()


# ── T009: wrong_tool_count tracking ──────────────────────────────────────────

class TestWrongToolCount:
    """Tests for wrong_tool_count: counts calls to tool names not in valid_tool_names set."""

    def test_known_tool_name_not_counted(self):
        """Calls to names in valid_tool_names set do not increment wrong_tool_count."""
        from toolset_tracker import ToolsetTracker  # noqa — will fail until T007 implemented

        tracker = ToolsetTracker(valid_tool_names={"iris_compile", "iris_doc", "iris_execute"})
        tracker.record_tool_call("iris_compile")
        tracker.record_tool_call("iris_doc")
        assert tracker.wrong_tool_count == 0

    def test_unknown_tool_name_increments_count(self):
        """Calls to names NOT in valid_tool_names increment wrong_tool_count."""
        from toolset_tracker import ToolsetTracker  # noqa

        tracker = ToolsetTracker(valid_tool_names={"iris_compile", "iris_doc"})
        tracker.record_tool_call("iris_symbols_local")  # removed in nostub/merged
        tracker.record_tool_call("debug_capture_packet")  # removed in merged
        assert tracker.wrong_tool_count == 2

    def test_mixed_calls_counted_correctly(self):
        """Mix of valid and invalid calls — only invalid ones count."""
        from toolset_tracker import ToolsetTracker  # noqa

        tracker = ToolsetTracker(valid_tool_names={"iris_compile", "iris_execute"})
        tracker.record_tool_call("iris_compile")       # valid
        tracker.record_tool_call("ghost_tool")          # invalid
        tracker.record_tool_call("iris_execute")        # valid
        tracker.record_tool_call("iris_symbols_local")  # invalid
        assert tracker.wrong_tool_count == 2
        assert tracker.total_tool_calls == 4

    def test_empty_valid_set_counts_all(self):
        """If valid_tool_names is empty, all calls are wrong (edge case)."""
        from toolset_tracker import ToolsetTracker  # noqa

        tracker = ToolsetTracker(valid_tool_names=set())
        tracker.record_tool_call("iris_compile")
        assert tracker.wrong_tool_count == 1

    def test_reset_clears_counts(self):
        """reset() clears all counts for a new task."""
        from toolset_tracker import ToolsetTracker  # noqa

        tracker = ToolsetTracker(valid_tool_names={"iris_compile"})
        tracker.record_tool_call("bad_tool")
        assert tracker.wrong_tool_count == 1
        tracker.reset()
        assert tracker.wrong_tool_count == 0
        assert tracker.total_tool_calls == 0
