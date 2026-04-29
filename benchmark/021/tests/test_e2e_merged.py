"""T028: E2E test — merged condition run produces scores.json with condition=merged.

Requires live IRIS: IRIS_HOST, IRIS_WEB_PORT, and iris-dev binary on PATH.
Uses iris-dev-iris container (iris-devtester named container — Constitution Principle IV).

Skip: automatically skipped when IRIS_HOST is not set.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(
    not os.environ.get("IRIS_HOST"),
    reason="IRIS_HOST not set — skipping E2E merged test (requires live iris-dev-iris)"
)


@pytest.fixture(scope="module")
def merged_scores():
    """Run GEN category under merged condition and return scores.json."""
    env = os.environ.copy()
    env["IRIS_TOOLSET"] = "merged"

    result = subprocess.run(
        [
            sys.executable, "-m", "benchmark._021.runner",
            "--toolset", "merged",
            "--categories", "GEN",
            "--path", "A",
        ],
        capture_output=True,
        text=True,
        env=env,
        timeout=600,
        cwd=str(Path(__file__).parent.parent.parent.parent),
    )

    assert result.returncode == 0, \
        f"Merged runner failed:\nSTDOUT: {result.stdout}\nSTDERR: {result.stderr}"

    results_dir = Path(__file__).parent.parent / "results"
    scores_files = sorted(results_dir.glob("*/scores.json"), key=os.path.getmtime)
    assert scores_files, "No scores.json found after merged run"

    with open(scores_files[-1]) as f:
        return json.load(f)


class TestMergedRun:
    """Verify merged condition: condition=merged, stub_error_count=0, scores valid."""

    def test_condition_is_merged(self, merged_scores):
        """Run-level condition field must be merged."""
        assert merged_scores.get("condition") == "merged", \
            f"Expected condition=merged, got {merged_scores.get('condition')}"

    def test_all_tasks_have_merged_condition(self, merged_scores):
        """Every task entry must have condition=merged."""
        for task in merged_scores.get("tasks", []):
            assert task.get("condition") == "merged", \
                f"Task {task.get('task_id')} has condition={task.get('condition')}"

    def test_stub_error_count_zero(self, merged_scores):
        """stub_error_count must be 0 (stubs removed in merged, SC-005)."""
        for task in merged_scores.get("tasks", []):
            assert task.get("stub_error_count", 0) == 0, \
                f"Task {task.get('task_id')} has stub_error_count={task.get('stub_error_count')}"

    def test_scores_in_valid_range(self, merged_scores):
        """All scores must be 0–3."""
        for task in merged_scores.get("tasks", []):
            score = task.get("score", -1)
            assert 0 <= score <= 3, \
                f"Score {score} out of range for {task.get('task_id')}"

    def test_required_fields_present(self, merged_scores):
        """All extended metric fields must be present."""
        for task in merged_scores.get("tasks", []):
            for field in ("condition", "tool_call_count", "stub_error_count", "wrong_tool_count"):
                assert field in task, \
                    f"Field '{field}' missing from task {task.get('task_id')}"

    def test_wall_clock_recorded(self, merged_scores):
        """wall_clock_seconds must be present in run metadata (SC-006)."""
        assert "wall_clock_seconds" in merged_scores, \
            f"wall_clock_seconds missing from merged run metadata"
