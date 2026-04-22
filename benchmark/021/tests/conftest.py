"""Add runner directory to sys.path for test imports."""
import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "runner"))
