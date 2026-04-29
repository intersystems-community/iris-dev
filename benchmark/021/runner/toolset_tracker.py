"""ToolsetTracker: tracks wrong_tool_count and total_tool_calls per task run.

After spawning iris-dev, the benchmark client calls tools/list to get the
registered tool names for the active condition (valid_tool_names). For each
agent tool call during the run, the call name is checked against this set.
Calls to names not in the set increment wrong_tool_count (FR-002).
"""


class ToolsetTracker:
    """Tracks tool call metrics for a single benchmark task run."""

    def __init__(self, valid_tool_names: set):
        self.valid_tool_names = set(valid_tool_names)
        self.total_tool_calls = 0
        self.wrong_tool_count = 0

    def record_tool_call(self, tool_name: str) -> None:
        """Record a tool call. Increments wrong_tool_count if name not in valid set."""
        self.total_tool_calls += 1
        if tool_name not in self.valid_tool_names:
            self.wrong_tool_count += 1

    def reset(self) -> None:
        """Reset counters for a new task run."""
        self.total_tool_calls = 0
        self.wrong_tool_count = 0

    def to_dict(self) -> dict:
        """Return metrics as a dict for inclusion in task result."""
        return {
            "total_tool_calls": self.total_tool_calls,
            "wrong_tool_count": self.wrong_tool_count,
        }
