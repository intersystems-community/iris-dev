"""T010: E2E test — baseline condition run produces scores.json with required fields.

Requires live IRIS: IRIS_HOST, IRIS_WEB_PORT, and iris-dev binary on PATH.
Uses iris-dev-iris container (iris-devtester named container — Constitution Principle IV).

Run: pytest benchmark/021/tests/test_e2e_baseline.py -v
Skip: automatically skipped when IRIS_HOST is not set.
"""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(
    not os.environ.get("IRIS_HOST"),
    reason="IRIS_HOST not set — skipping E2E baseline test (requires live iris-dev-iris container)"
)


@pytest.fixture(scope="module")
def baseline_scores():
    """Run a minimal baseline condition (GEN category only, 5 tasks) and return scores.json."""
    with tempfile.TemporaryDirectory() as tmpdir:
        env = os.environ.copy()
        env["IRIS_TOOLSET"] = "baseline"

        result = subprocess.run(
            [
                sys.executable, "-m", "benchmark._021.runner",
                "--toolset", "baseline",
                "--categories", "GEN",
                "--path", "A",
            ],
            capture_output=True,
            text=True,
            env=env,
            timeout=600,
            cwd=str(Path(__file__).parent.parent.parent.parent),  # repo root
        )

        assert result.returncode == 0, \
            f"Baseline runner failed:\nSTDOUT: {result.stdout}\nSTDERR: {result.stderr}"

        # Find the most recent scores.json
        results_dir = Path(__file__).parent.parent / "results"
        scores_files = sorted(results_dir.glob("*/scores.json"), key=os.path.getmtime)
        assert scores_files, "No scores.json found after baseline run"

        with open(scores_files[-1]) as f:
            return json.load(f)


class TestBaselineRun:
    """Verify baseline condition run produces correctly structured scores.json (US1)."""

    def test_tasks_completed(self, baseline_scores):
        """Baseline run must complete all GEN tasks (5 tasks × path A)."""
        tasks = baseline_scores.get("tasks", [])
        assert len(tasks) >= 5, f"Expected ≥5 GEN tasks, got {len(tasks)}"

    def test_condition_field_present(self, baseline_scores):
        """Every task entry must have condition=baseline."""
        tasks = baseline_scores.get("tasks", [])
        for task in tasks:
            assert "condition" in task, f"'condition' missing in task {task.get('task_id')}"
            assert task["condition"] == "baseline", \
                f"Expected condition=baseline, got {task['condition']}"

    def test_tool_call_count_present(self, baseline_scores):
        """Every task must have a non-negative tool_call_count."""
        for task in baseline_scores.get("tasks", []):
            assert "tool_call_count" in task, \
                f"'tool_call_count' missing in {task.get('task_id')}"
            assert task["tool_call_count"] >= 0

    def test_stub_error_count_present(self, baseline_scores):
        """Every task must have stub_error_count field."""
        for task in baseline_scores.get("tasks", []):
            assert "stub_error_count" in task, \
                f"'stub_error_count' missing in {task.get('task_id')}"

    def test_wrong_tool_count_present(self, baseline_scores):
        """Every task must have wrong_tool_count field."""
        for task in baseline_scores.get("tasks", []):
            assert "wrong_tool_count" in task, \
                f"'wrong_tool_count' missing in {task.get('task_id')}"

    def test_run_has_condition_metadata(self, baseline_scores):
        """Top-level run must have condition and wall_clock_seconds fields."""
        assert "condition" in baseline_scores, \
            f"Top-level 'condition' missing. Keys: {list(baseline_scores.keys())}"
        assert baseline_scores["condition"] == "baseline"
        assert "wall_clock_seconds" in baseline_scores, \
            "'wall_clock_seconds' missing from run metadata"

    def test_scores_in_valid_range(self, baseline_scores):
        """All scores must be 0–3."""
        for task in baseline_scores.get("tasks", []):
            score = task.get("score", -1)
            assert 0 <= score <= 3, \
                f"Score {score} out of range for task {task.get('task_id')}"
