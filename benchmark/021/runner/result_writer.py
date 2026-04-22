"""Write benchmark results incrementally to JSON and generate HTML report."""
import json
import os
import datetime
from pathlib import Path


class ResultWriter:
    def __init__(self):
        ts = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H-%M-%SZ")
        base = Path(__file__).parent.parent / "results" / ts
        base.mkdir(parents=True, exist_ok=True)
        self.run_dir = str(base)
        self.scores_path = str(base / "scores.json")
        self.report_path = str(base / "report.html")
        self._run = {
            "run_id": ts,
            "iris_dev_version": _get_version(),
            "tasks": [],
            "summary": {},
        }
        self._flush()

    def record(self, task_id: str, category: str, path: str, harness: str,
               scored: dict, result: dict):
        entry = {
            "task_id": task_id,
            "category": category,
            "path": path,
            "harness": harness,
            "score": scored["score"],
            "reasoning": scored.get("reasoning", ""),
            "tool_call_count": result.get("tool_call_count", 0),
            "scm_elicitation_triggered": _scm_triggered(result.get("transcript", [])),
        }
        self._run["tasks"].append(entry)
        self._flush()

    def finalize(self):
        self._run["summary"] = _compute_summary(self._run["tasks"])
        self._flush()
        self._write_html()
        print(f"scores.json  → {self.scores_path}")
        print(f"report.html  → {self.report_path}")

    def _flush(self):
        with open(self.scores_path, "w") as f:
            json.dump(self._run, f, indent=2)

    def _write_html(self):
        from .report import generate_report
        generate_report(self.scores_path, self.report_path)


def _get_version() -> str:
    import subprocess
    try:
        r = subprocess.run(["iris-dev", "--version"], capture_output=True, text=True)
        return r.stdout.strip().split()[-1]
    except Exception:
        return "unknown"


def _scm_triggered(transcript: list) -> bool:
    return any(
        t.get("tool_name") == "iris_source_control" or
        "elicitation" in str(t.get("tool_result", "")).lower()
        for t in transcript
    )


def _compute_summary(tasks: list) -> dict:
    if not tasks:
        return {}

    scores_a = [t["score"] for t in tasks if t["path"] == "A"]
    scores_b = [t["score"] for t in tasks if t["path"] == "B"]

    by_category = {}
    for t in tasks:
        cat = t["category"]
        if cat not in by_category:
            by_category[cat] = {"A": [], "B": []}
        by_category[cat][t["path"]].append(t["score"])

    return {
        "mean_score_path_a": _mean(scores_a),
        "mean_score_path_b": _mean(scores_b),
        "task_count": len(tasks),
        "by_category": {
            cat: {
                "path_a": _mean(v["A"]),
                "path_b": _mean(v["B"]),
            }
            for cat, v in by_category.items()
        },
    }


def _mean(vals: list) -> float:
    return round(sum(vals) / len(vals), 2) if vals else 0.0
