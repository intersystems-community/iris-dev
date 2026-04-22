"""T006 — Unit tests for namespace.py (requires live IRIS)."""
import os
import pytest

pytestmark = pytest.mark.skipif(
    not os.environ.get("IRIS_HOST"),
    reason="IRIS_HOST not set — skipping namespace tests"
)


def test_ensure_benchmark_namespace():
    from namespace import ensure_benchmark_namespace
    ensure_benchmark_namespace()  # should not raise


def test_wipe_benchmark_namespace():
    from namespace import ensure_benchmark_namespace, wipe_benchmark_namespace
    ensure_benchmark_namespace()
    wipe_benchmark_namespace()  # should not raise


def test_idempotent_create():
    from namespace import ensure_benchmark_namespace
    ensure_benchmark_namespace()
    ensure_benchmark_namespace()  # second call should be a no-op, not error
