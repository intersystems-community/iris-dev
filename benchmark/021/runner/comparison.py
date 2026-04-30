"""T038–T040: Multi-condition comparison report generator.

generate_comparison(baseline, nostub, merged) -> ComparisonReport dict

Hypothesis verdict logic (SC-002, SC-003, SC-004):
  confirmed: merged mean_score >= baseline mean_score + 0.1
             AND merged tool_call_count_mean <= baseline tool_call_count_mean
  rejected:  otherwise
  (tool_count regression is noted but does not override score confirmation)
"""
from __future__ import annotations

import json
import statistics
from pathlib import Path
from typing import Any


HYPOTHESIS_THRESHOLD = 0.1  # SC-002: merged must beat baseline by at least this


def _summarise(scores_data: dict) -> dict:
    """Compute per-condition summary stats from a scores.json dict."""
    tasks = scores_data.get("tasks", [])
    if not tasks:
        return {
            "mean_score": 0.0,
            "tool_call_count_mean": 0.0,
            "total_stub_errors": 0,
            "total_wrong_tool_calls": 0,
            "wall_clock_seconds": scores_data.get("wall_clock_seconds", 0.0),
            "task_count": 0,
            "by_category": {},
        }

    score_vals = [t.get("score", 0) for t in tasks]
    tool_vals = [t.get("tool_call_count", 0) for t in tasks]

    by_category: dict[str, list[float]] = {}
    for t in tasks:
        cat = t.get("category", "?")
        by_category.setdefault(cat, []).append(t.get("score", 0))

    return {
        "mean_score": round(statistics.mean(score_vals), 3),
        "tool_call_count_mean": round(statistics.mean(tool_vals), 2),
        "total_stub_errors": sum(t.get("stub_error_count", 0) for t in tasks),
        "total_wrong_tool_calls": sum(t.get("wrong_tool_count", 0) for t in tasks),
        "wall_clock_seconds": scores_data.get("wall_clock_seconds", 0.0),
        "task_count": len(tasks),
        "by_category": {
            cat: round(statistics.mean(scores), 3)
            for cat, scores in by_category.items()
        },
    }


def _detect_regressions(baseline_data: dict, merged_data: dict) -> list[dict]:
    """Find tasks that scored 3 in baseline but < 2 in merged (SC-004)."""
    baseline_scores = {t["task_id"]: t["score"] for t in baseline_data.get("tasks", [])}
    regressions = []
    for task in merged_data.get("tasks", []):
        task_id = task.get("task_id", "")
        merged_score = task.get("score", 0)
        baseline_score = baseline_scores.get(task_id)
        if baseline_score is not None and baseline_score == 3 and merged_score < 2:
            regressions.append({
                "task_id": task_id,
                "baseline_score": baseline_score,
                "merged_score": merged_score,
            })
    return regressions


def generate_comparison(
    baseline_data: dict,
    nostub_data: dict,
    merged_data: dict,
) -> dict:
    """Generate a three-way comparison report.

    Args:
        baseline_data: parsed scores.json for baseline condition
        nostub_data: parsed scores.json for nostub condition
        merged_data: parsed scores.json for merged condition

    Returns:
        ComparisonReport dict matching contracts/runner-cli.md schema
    """
    baseline_stats = _summarise(baseline_data)
    nostub_stats = _summarise(nostub_data)
    merged_stats = _summarise(merged_data)

    score_diff = merged_stats["mean_score"] - baseline_stats["mean_score"]
    tool_count_ok = (
        merged_stats["tool_call_count_mean"] <= baseline_stats["tool_call_count_mean"]
    )
    score_ok = score_diff >= HYPOTHESIS_THRESHOLD

    hypothesis_result = "confirmed" if score_ok else "rejected"

    regressions = _detect_regressions(baseline_data, merged_data)

    notes = [
        "Fixed condition order: baseline → nostub → merged. "
        "LLM cache warmup is a known limitation of the pilot study design.",
        "5 tasks per category — directional signal only, not statistically significant.",
    ]

    if not tool_count_ok:
        notes.append(
            f"Warning: merged tool_call_count_mean ({merged_stats['tool_call_count_mean']}) "
            f"> baseline ({baseline_stats['tool_call_count_mean']}). "
            "SC-003 not met — tool count did not decrease."
        )

    if regressions:
        notes.append(
            f"{len(regressions)} regression(s) detected: "
            + ", ".join(r["task_id"] for r in regressions)
        )

    return {
        "conditions": {
            "baseline": baseline_stats,
            "nostub": nostub_stats,
            "merged": merged_stats,
        },
        "score_diff_merged_vs_baseline": round(score_diff, 3),
        "hypothesis_result": hypothesis_result,
        "regressions": regressions,
        "notes": notes,
    }


def generate_comparison_from_dirs(
    baseline_dir: str | Path,
    nostub_dir: str | Path,
    merged_dir: str | Path,
    output_path: str | Path | None = None,
) -> dict:
    """Load scores.json from three result directories and generate comparison.

    Args:
        baseline_dir: path to baseline condition result directory
        nostub_dir: path to nostub condition result directory
        merged_dir: path to merged condition result directory
        output_path: optional path to write comparison.json

    Returns:
        ComparisonReport dict
    """
    def load(d: str | Path) -> dict:
        scores_file = Path(d) / "scores.json"
        if not scores_file.exists():
            raise FileNotFoundError(f"scores.json not found in {d}")
        with open(scores_file) as f:
            return json.load(f)

    result = generate_comparison(load(baseline_dir), load(nostub_dir), load(merged_dir))

    if output_path:
        with open(output_path, "w") as f:
            json.dump(result, f, indent=2)

    return result
