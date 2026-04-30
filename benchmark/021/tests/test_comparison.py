"""T035–T037: Unit tests for comparison report generation.

Tests written FIRST — must FAIL before T038–T040 are implemented.
"""
import json
import pytest


def make_scores(condition, mean_score, tool_call_count_mean=5.0,
                stub_errors=0, wrong_tools=0, tasks=None):
    """Helper: build a minimal scores.json dict for testing."""
    if tasks is None:
        tasks = [
            {
                "task_id": f"GEN-0{i+1}",
                "category": "GEN",
                "path": "A",
                "harness": "claude-code",
                "condition": condition,
                "score": round(mean_score),
                "reasoning": "ok",
                "tool_call_count": int(tool_call_count_mean),
                "stub_error_count": stub_errors,
                "wrong_tool_count": wrong_tools,
                "scm_elicitation_triggered": False,
            }
            for i in range(5)
        ]
    return {
        "run_id": f"2026-04-29T00-00-00Z",
        "condition": condition,
        "iris_dev_version": "0.4.7",
        "wall_clock_seconds": 120.0,
        "tasks": tasks,
        "summary": {},
    }


class TestComparisonGeneration:
    """T035: comparison.json structure matches contract schema."""

    def test_comparison_has_required_keys(self, tmp_path):
        """comparison.json must contain conditions, hypothesis_result, regressions, notes."""
        from comparison import generate_comparison  # noqa — will fail until T038

        baseline = make_scores("baseline", 2.4, tool_call_count_mean=5)
        nostub = make_scores("nostub", 2.6, tool_call_count_mean=4)
        merged = make_scores("merged", 2.8, tool_call_count_mean=4)

        result = generate_comparison(baseline, nostub, merged)

        assert "conditions" in result, f"'conditions' missing. Keys: {list(result.keys())}"
        assert "hypothesis_result" in result
        assert "regressions" in result
        assert "notes" in result

    def test_conditions_have_per_condition_stats(self, tmp_path):
        """Each condition entry must have mean_score and tool_call_count_mean."""
        from comparison import generate_comparison  # noqa

        baseline = make_scores("baseline", 2.4)
        nostub = make_scores("nostub", 2.6)
        merged = make_scores("merged", 2.8)

        result = generate_comparison(baseline, nostub, merged)

        for cond in ("baseline", "nostub", "merged"):
            assert cond in result["conditions"], f"'{cond}' missing from conditions"
            stats = result["conditions"][cond]
            assert "mean_score" in stats, f"mean_score missing for {cond}"
            assert "tool_call_count_mean" in stats
            assert "total_stub_errors" in stats
            assert "total_wrong_tool_calls" in stats


class TestHypothesisVerdict:
    """T036: hypothesis_result=confirmed when C3 mean_score >= C1 mean_score + 0.1."""

    def test_confirmed_when_merged_beats_baseline_by_threshold(self):
        """Merged 2.6 vs baseline 2.4 → confirmed (diff=0.2 >= 0.1)."""
        from comparison import generate_comparison  # noqa

        baseline = make_scores("baseline", 2.4, tool_call_count_mean=5)
        nostub = make_scores("nostub", 2.5, tool_call_count_mean=5)
        merged = make_scores("merged", 2.6, tool_call_count_mean=4)

        result = generate_comparison(baseline, nostub, merged)
        assert result["hypothesis_result"] == "confirmed", \
            f"Expected confirmed, got {result['hypothesis_result']}"

    def test_rejected_when_merged_does_not_beat_baseline(self):
        """Merged 2.4 vs baseline 2.4 → rejected (diff=0.0 < 0.1)."""
        from comparison import generate_comparison  # noqa

        baseline = make_scores("baseline", 2.4)
        nostub = make_scores("nostub", 2.4)
        merged = make_scores("merged", 2.4)

        result = generate_comparison(baseline, nostub, merged)
        assert result["hypothesis_result"] == "rejected", \
            f"Expected rejected, got {result['hypothesis_result']}"

    def test_rejected_when_merged_worse_than_baseline(self):
        """Merged 2.2 vs baseline 2.4 → rejected."""
        from comparison import generate_comparison  # noqa

        baseline = make_scores("baseline", 2.4)
        nostub = make_scores("nostub", 2.3)
        merged = make_scores("merged", 2.2)

        result = generate_comparison(baseline, nostub, merged)
        assert result["hypothesis_result"] == "rejected"

    def test_confirmed_also_requires_lower_tool_count(self):
        """Confirmed requires merged tool_call_count <= baseline (SC-003)."""
        from comparison import generate_comparison  # noqa

        # merged scores higher BUT uses more tool calls → should still be confirmed for score,
        # but notes section should flag the tool_count regression
        baseline = make_scores("baseline", 2.4, tool_call_count_mean=4)
        nostub = make_scores("nostub", 2.5, tool_call_count_mean=4)
        merged = make_scores("merged", 2.6, tool_call_count_mean=6)  # worse tool count

        result = generate_comparison(baseline, nostub, merged)
        # hypothesis confirmed on score but tool_count_regression flagged
        assert result["hypothesis_result"] == "confirmed"
        assert any("tool" in note.lower() for note in result.get("notes", [])), \
            "Notes should flag tool_call_count regression when merged > baseline"


class TestRegressionDetection:
    """T037: regressions list populated when task scored 3 in baseline but <2 in merged."""

    def test_regression_detected(self):
        """Task scored 3 in baseline and 1 in merged → regression."""
        from comparison import generate_comparison  # noqa

        baseline_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "baseline",
             "score": 3, "tool_call_count": 4, "stub_error_count": 0, "wrong_tool_count": 0},
        ]
        merged_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "merged",
             "score": 1, "tool_call_count": 4, "stub_error_count": 0, "wrong_tool_count": 0},
        ]

        baseline = make_scores("baseline", 3.0, tasks=baseline_tasks)
        nostub = make_scores("nostub", 3.0)
        merged = make_scores("merged", 1.0, tasks=merged_tasks)

        result = generate_comparison(baseline, nostub, merged)
        assert len(result["regressions"]) == 1, \
            f"Expected 1 regression, got {len(result['regressions'])}: {result['regressions']}"
        assert result["regressions"][0]["task_id"] == "GEN-01"

    def test_no_regression_when_score_stays_same(self):
        """Task scored 3 in baseline and 3 in merged → no regression."""
        from comparison import generate_comparison  # noqa

        baseline_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "baseline",
             "score": 3, "tool_call_count": 4, "stub_error_count": 0, "wrong_tool_count": 0},
        ]
        merged_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "merged",
             "score": 3, "tool_call_count": 3, "stub_error_count": 0, "wrong_tool_count": 0},
        ]

        baseline = make_scores("baseline", 3.0, tasks=baseline_tasks)
        nostub = make_scores("nostub", 3.0)
        merged = make_scores("merged", 3.0, tasks=merged_tasks)

        result = generate_comparison(baseline, nostub, merged)
        assert len(result["regressions"]) == 0, \
            f"Expected 0 regressions, got {result['regressions']}"

    def test_score_2_not_regression(self):
        """Threshold is <2 not <=2: task scored 3 → 2 is NOT a regression."""
        from comparison import generate_comparison  # noqa

        baseline_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "baseline",
             "score": 3, "tool_call_count": 4, "stub_error_count": 0, "wrong_tool_count": 0},
        ]
        merged_tasks = [
            {"task_id": "GEN-01", "category": "GEN", "condition": "merged",
             "score": 2, "tool_call_count": 3, "stub_error_count": 0, "wrong_tool_count": 0},
        ]

        baseline = make_scores("baseline", 3.0, tasks=baseline_tasks)
        nostub = make_scores("nostub", 3.0)
        merged = make_scores("merged", 2.0, tasks=merged_tasks)

        result = generate_comparison(baseline, nostub, merged)
        assert len(result["regressions"]) == 0, \
            "Score 3→2 should NOT be a regression (threshold is <2)"
