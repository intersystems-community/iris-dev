"""Entry point for benchmark/021 runner. Works around the digit-named package issue."""
import sys, os
# Make benchmark/021 importable as a package by inserting it directly
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '021'))
# Rewrite relative imports in runner modules to absolute
import runner.__main__ as _main  # noqa
if __name__ == '__main__':
    _main.main()
