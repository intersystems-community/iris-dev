"""Shared fixtures for integration tests — imports from conftest_022."""
import sys
import os
sys.path.insert(0, os.path.dirname(__file__))

# Re-export all fixtures from conftest_022 so pytest discovers them
from conftest_022 import iris_env, seed_test_class  # noqa: F401
