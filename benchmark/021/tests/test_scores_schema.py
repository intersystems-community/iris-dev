"""T011: Unit tests for extended scores.json schema (condition + new metrics fields).

Tests the result_writer.record() method produces the correct per-task fields (FR-002).
"""
import json
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "runner"))


class TestScoresSchema:
    """Verify result_writer produces required fields in task entries."""

    def _make_entry(self, condition="baseline", tool_call_count=4,
                    stub_error_count=0, wrong_tool_count=0):
        """Directly call the entry-building logic from result_writer."""
        from result_writer import ResultWriter

        # Patch filesystem ops so no actual files are created
        import unittest.mock as mock
        with mock.patch("result_writer._get_version", return_value="0.4.7"), \
             mock.patch.object(ResultWriter, "_flush"), \
             mock.patch.object(ResultWriter, "_write_html"), \
             mock.patch("result_writer.Path") as mock_path_cls:

            mock_path_cls.return_value.__truediv__ = lambda s, x: Path(tempfile.gettempdir()) / x
            mock_path_cls.return_value.parent.parent = Path(tempfile.gettempdir())

            writer = ResultWriter.__new__(ResultWriter)
            writer._run = {"run_id": "test", "iris_dev_version": "0.4.7", "tasks": [], "summary": {}}
            writer.run_dir = tempfile.gettempdir()
            writer.scores_path = os.path.join(tempfile.gettempdir(), "scores.json")
            writer.report_path = os.path.join(tempfile.gettempdir(), "report.html")

            writer.record(
                task_id="GEN-01",
                category="GEN",
                path="A",
                harness="claude-code",
                scored={"score": 3, "reasoning": "ok"},
                result={
                    "tool_call_count": tool_call_count,
                    "stub_error_count": stub_error_count,
                    "wrong_tool_count": wrong_tool_count,
                    "transcript": [],
                },
                condition=condition,
            )
            return writer._run["tasks"][0]

    def test_task_has_condition_field(self):
        """Each task entry must include a 'condition' field (FR-002)."""
        entry = self._make_entry(condition="baseline")
        assert "condition" in entry, f"'condition' missing. Keys: {list(entry.keys())}"
        assert entry["condition"] == "baseline"

    def test_task_has_tool_call_count(self):
        """Each task entry must include 'tool_call_count' (FR-002)."""
        entry = self._make_entry(tool_call_count=5)
        assert "tool_call_count" in entry, f"'tool_call_count' missing. Keys: {list(entry.keys())}"
        assert entry["tool_call_count"] == 5

    def test_task_has_stub_error_count(self):
        """Each task entry must include 'stub_error_count' (FR-002)."""
        entry = self._make_entry(stub_error_count=2)
        assert "stub_error_count" in entry, f"'stub_error_count' missing. Keys: {list(entry.keys())}"
        assert entry["stub_error_count"] == 2

    def test_task_has_wrong_tool_count(self):
        """Each task entry must include 'wrong_tool_count' (FR-002)."""
        entry = self._make_entry(wrong_tool_count=1)
        assert "wrong_tool_count" in entry, f"'wrong_tool_count' missing. Keys: {list(entry.keys())}"
        assert entry["wrong_tool_count"] == 1

    def test_condition_metadata_stored(self):
        """set_condition_metadata() stores condition and wall_clock_seconds at run level."""
        import unittest.mock as mock
        from result_writer import ResultWriter

        with mock.patch("result_writer._get_version", return_value="0.4.7"), \
             mock.patch.object(ResultWriter, "_flush"), \
             mock.patch.object(ResultWriter, "_write_html"), \
             mock.patch("result_writer.Path"):

            writer = ResultWriter.__new__(ResultWriter)
            writer._run = {"run_id": "test", "tasks": [], "summary": {}}
            writer.set_condition_metadata("merged", 120.5)

            assert writer._run["condition"] == "merged"
            assert writer._run["wall_clock_seconds"] == 120.5

    def test_nostub_condition_roundtrips(self):
        """condition='nostub' is preserved correctly."""
        entry = self._make_entry(condition="nostub")
        assert entry["condition"] == "nostub"
