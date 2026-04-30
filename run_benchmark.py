"""Wrapper to run benchmark/021 as a package (021 starts with digit, can't use -m)."""
import sys
import importlib
import types

# Inject benchmark.021 as a importable package
import benchmark
pkg = types.ModuleType("benchmark.021")
pkg.__path__ = ["benchmark/021"]
pkg.__package__ = "benchmark.021"
sys.modules["benchmark.021"] = pkg

# Now load runner sub-packages
from benchmark._021 import runner  # noqa — this won't work either

