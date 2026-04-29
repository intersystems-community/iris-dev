"""benchmark/021 runner — path-aware agentic benchmark for iris-dev."""
import argparse
import os
import sys
import time


def parse_args():
    p = argparse.ArgumentParser(description="iris-dev path-aware benchmark runner")
    p.add_argument("--path", choices=["A", "B", "both"], default="both",
                   help="Which development path to benchmark (default: both)")
    p.add_argument("--categories", default=None,
                   help="Comma-separated task categories to run, e.g. GEN,MOD")
    p.add_argument("--task", default=None,
                   help="Run a single task by ID, e.g. GEN-01")
    p.add_argument("--harness", choices=["claude-code", "copilot", "both"], default="claude-code",
                   help="Which AI harness to use (default: claude-code)")
    p.add_argument("--toolset", choices=["baseline", "nostub", "merged"], default=None,
                   help="Tool set condition to run. Sets IRIS_TOOLSET env var for iris-dev. "
                        "If omitted, uses current IRIS_TOOLSET or defaults to baseline.")
    p.add_argument("--report-only", metavar="SCORES_JSON",
                   help="Generate report from an existing scores.json, skip running tasks")
    p.add_argument("--dry-run", action="store_true",
                   help="Load tasks and print plan without executing")
    return p.parse_args()


def check_env():
    required = ["IRIS_HOST", "IRIS_WEB_PORT"]
    missing = [k for k in required if not os.environ.get(k)]
    if missing:
        print(f"ERROR: missing required env vars: {', '.join(missing)}", file=sys.stderr)
        sys.exit(2)
    use_bedrock = bool(
        os.environ.get("CLAUDE_CODE_USE_BEDROCK") or
        os.environ.get("AWS_BEARER_TOKEN_BEDROCK") or
        os.environ.get("AWS_ACCESS_KEY_ID")
    )
    if not use_bedrock and not os.environ.get("ANTHROPIC_API_KEY"):
        print("WARNING: no API credentials found. Set ANTHROPIC_API_KEY or AWS Bedrock env vars.",
              file=sys.stderr)


def main():
    args = parse_args()

    if args.report_only:
        from .report import generate_report
        generate_report(args.report_only)
        return

    # Set IRIS_TOOLSET before any subprocess or discovery runs
    if args.toolset:
        os.environ["IRIS_TOOLSET"] = args.toolset

    check_env()

    from .task_loader import load_tasks
    tasks = load_tasks(
        path_filter=args.path,
        category_filter=args.categories.split(",") if args.categories else None,
        task_id=args.task,
    )

    if args.dry_run:
        print(f"Dry run: {len(tasks)} task(s) would run")
        for t in tasks:
            print(f"  {t['id']:10s}  path={t.get('path','both'):4s}  {t['description'][:60]}")
        return

    print(f"Running {len(tasks)} task(s) | path={args.path} | harness={args.harness}")

    active_condition = os.environ.get("IRIS_TOOLSET", "baseline")
    print(f"Condition: {active_condition}")

    # Reset namespace before the condition run (FR-001b)
    from .namespace import reset_benchmark_namespace, ensure_benchmark_namespace
    try:
        reset_benchmark_namespace()
        print(f"BENCHMARK namespace reset for condition={active_condition}")
    except Exception as e:
        # Fallback: ensure namespace exists if reset fails (e.g. first run)
        try:
            ensure_benchmark_namespace()
        except Exception:
            pass
        print(f"Note: namespace reset skipped ({e.__class__.__name__}: {e}), using ensure instead")

    from .result_writer import ResultWriter
    writer = ResultWriter()

    paths = (["A", "B"] if args.path == "both" else [args.path])
    harnesses = (["claude-code", "copilot"] if args.harness == "both" else [args.harness])

    condition_start = time.time()

    for path in paths:
        for harness in harnesses:
            if harness == "claude-code":
                from .claude_code import run_task
            else:
                from .copilot import run_task  # noqa: F811

            for task in tasks:
                if task.get("path", "both") not in ("both", path):
                    continue
                print(f"  [{path}/{harness}] {task['id']} ...", end=" ", flush=True)
                from .fixtures import apply_fixtures
                apply_fixtures(task.get("fixtures", []))
                result = run_task(task, path)
                from .judge import score_result
                scored = score_result(task, result)
                writer.record(task["id"], task["category"], path, harness, scored, result,
                               condition=active_condition)
                from .namespace import wipe_benchmark_namespace
                wipe_benchmark_namespace()
                print(f"score={scored['score']}")

    condition_wall_clock = round(time.time() - condition_start, 1)
    writer.set_condition_metadata(active_condition, condition_wall_clock)
    writer.finalize()
    print(f"\nResults: {writer.run_dir} (wall_clock={condition_wall_clock}s)")


if __name__ == "__main__":
    main()
