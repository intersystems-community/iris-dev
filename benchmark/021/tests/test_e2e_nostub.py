"""T016: E2E test — nostub condition run produces stub_error_count=0.

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
    reason="IRIS_HOST not set — skipping E2E nostub test (requires live iris-dev-iris)"
)


@pytest.fixture(scope="module")
def nostub_scores():
    """Run GEN category under nostub condition and return scores.json."""
    env = os.environ.copy()
    env["IRIS_TOOLSET"] = "nostub"

    result = subprocess.run(
        [
            sys.executable, "-m", "benchmark._021.runner",
            "--toolset", "nostub",
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
        f"Nostub runner failed:\nSTDOUT: {result.stdout}\nSTDERR: {result.stderr}"

    results_dir = Path(__file__).parent.parent / "results"
    scores_files = sorted(results_dir.glob("*/scores.json"), key=os.path.getmtime)
    assert scores_files, "No scores.json found after nostub run"

    with open(scores_files[-1]) as f:
        return json.load(f)


class TestNostubRun:
    """Verify nostub condition: stub_error_count=0, condition=nostub, tool counts reduced."""

    def test_condition_is_nostub(self, nostub_scores):
        """Run-level condition field must be nostub."""
        assert nostub_scores.get("condition") == "nostub", \
            f"Expected condition=nostub, got {nostub_scores.get('condition')}"

    def test_all_tasks_have_nostub_condition(self, nostub_scores):
        """Every task entry must have condition=nostub."""
        for task in nostub_scores.get("tasks", []):
            assert task.get("condition") == "nostub", \
                f"Task {task.get('task_id')} has condition={task.get('condition')}"

    def test_stub_error_count_zero(self, nostub_scores):
        """stub_error_count must be 0 for all tasks (SC-005, FR-004–006)."""
        for task in nostub_scores.get("tasks", []):
            assert task.get("stub_error_count", 0) == 0, \
                f"Task {task.get('task_id')} has stub_error_count={task.get('stub_error_count')}"

    def test_scores_in_valid_range(self, nostub_scores):
        """All scores must be 0–3."""
        for task in nostub_scores.get("tasks", []):
            score = task.get("score", -1)
            assert 0 <= score <= 3, \
                f"Score {score} out of range for {task.get('task_id')}"

    def test_required_fields_present(self, nostub_scores):
        """All extended metric fields must be present."""
        for task in nostub_scores.get("tasks", []):
            for field in ("condition", "tool_call_count", "stub_error_count", "wrong_tool_count"):
                assert field in task, \
                    f"Field '{field}' missing from task {task.get('task_id')}"
