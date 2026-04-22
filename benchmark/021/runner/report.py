"""Generate HTML report from scores.json."""
import json
import os
from pathlib import Path


TEMPLATE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>iris-dev Path Benchmark — {run_id}</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
         background: #0d1117; color: #e6edf3; margin: 0; padding: 2rem; }}
  h1 {{ color: #58a6ff; }}
  h2 {{ color: #8b949e; font-size: 1rem; font-weight: 400; margin-top: 0; }}
  .meta {{ color: #8b949e; font-size: 0.85rem; margin-bottom: 2rem; }}
  .grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 2rem; margin: 2rem 0; }}
  .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1.5rem; }}
  .score-big {{ font-size: 3rem; font-weight: 700; color: #3fb950; }}
  .score-b {{ color: #f85149; }}
  table {{ width: 100%; border-collapse: collapse; margin-top: 1rem; }}
  th {{ text-align: left; padding: 0.5rem; background: #1c2128; color: #8b949e;
       font-size: 0.8rem; text-transform: uppercase; }}
  td {{ padding: 0.5rem; border-bottom: 1px solid #21262d; font-size: 0.9rem; }}
  .s3 {{ color: #3fb950; }} .s2 {{ color: #d29922; }}
  .s1 {{ color: #f85149; }} .s0 {{ color: #6e7681; }}
  .bar-wrap {{ background: #21262d; border-radius: 4px; height: 8px; margin: 4px 0; }}
  .bar {{ height: 8px; border-radius: 4px; }}
  .bar-a {{ background: #3fb950; }} .bar-b {{ background: #f85149; }}
  .path-a {{ color: #3fb950; }} .path-b {{ color: #f85149; }}
</style>
</head>
<body>
<h1>iris-dev Path-Aware Benchmark</h1>
<h2>Run {run_id} &nbsp;·&nbsp; iris-dev {version}</h2>
<div class="meta">
  {task_count} tasks &nbsp;·&nbsp;
  Path A (Local+Atelier) vs Path B (ISFS Only)
</div>

<div class="grid">
  <div class="card">
    <div style="color:#8b949e;font-size:.8rem;text-transform:uppercase">Mean Score — Path A</div>
    <div class="score-big">{mean_a}</div>
    <div style="color:#8b949e">Local Files + Atelier (blessed path)</div>
  </div>
  <div class="card">
    <div style="color:#8b949e;font-size:.8rem;text-transform:uppercase">Mean Score — Path B</div>
    <div class="score-big score-b">{mean_b}</div>
    <div style="color:#8b949e">ISFS Only (legacy/remote-only)</div>
  </div>
</div>

<div class="card" style="margin-bottom:2rem">
  <h3 style="margin-top:0;color:#58a6ff">Score by Category</h3>
  {category_bars}
</div>

<div class="card">
  <h3 style="margin-top:0;color:#58a6ff">All Tasks</h3>
  <table>
    <tr><th>Task</th><th>Cat</th><th>Path</th><th>Harness</th><th>Score</th><th>Tool Calls</th><th>Reasoning</th></tr>
    {task_rows}
  </table>
</div>
</body>
</html>"""


def generate_report(scores_path: str, output_path: str = None):
    with open(scores_path) as f:
        run = json.load(f)

    if output_path is None:
        output_path = str(Path(scores_path).parent / "report.html")

    summary = run.get("summary", {})
    mean_a = summary.get("mean_score_path_a", 0)
    mean_b = summary.get("mean_score_path_b", 0)
    by_cat = summary.get("by_category", {})

    # category bars
    bars = []
    for cat, vals in sorted(by_cat.items()):
        pa = vals.get("path_a", 0)
        pb = vals.get("path_b", 0)
        bars.append(f"""
  <div style="margin:0.8rem 0">
    <div style="display:flex;justify-content:space-between;margin-bottom:2px">
      <span style="font-weight:600">{cat}</span>
      <span><span class="path-a">{pa}</span> vs <span class="path-b">{pb}</span></span>
    </div>
    <div class="bar-wrap"><div class="bar bar-a" style="width:{pa/3*100:.0f}%"></div></div>
    <div class="bar-wrap"><div class="bar bar-b" style="width:{pb/3*100:.0f}%"></div></div>
  </div>""")

    # task rows
    rows = []
    score_cls = {0: "s0", 1: "s1", 2: "s2", 3: "s3"}
    for t in run.get("tasks", []):
        sc = t["score"]
        rows.append(
            f'<tr>'
            f'<td>{t["task_id"]}</td>'
            f'<td>{t["category"]}</td>'
            f'<td class="path-{"a" if t["path"]=="A" else "b"}">{t["path"]}</td>'
            f'<td>{t["harness"]}</td>'
            f'<td class="{score_cls[sc]}" style="font-weight:700">{sc}</td>'
            f'<td>{t["tool_call_count"]}</td>'
            f'<td style="color:#8b949e;font-size:.8rem">{t.get("reasoning","")[:80]}</td>'
            f'</tr>'
        )

    html = TEMPLATE.format(
        run_id=run["run_id"],
        version=run.get("iris_dev_version", "?"),
        task_count=len(run.get("tasks", [])),
        mean_a=mean_a,
        mean_b=mean_b,
        category_bars="\n".join(bars) or "<p>No data yet</p>",
        task_rows="\n".join(rows) or "<tr><td colspan=7>No tasks yet</td></tr>",
    )

    with open(output_path, "w") as f:
        f.write(html)

    return output_path
