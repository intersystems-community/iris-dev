"""Apply task fixtures to the BENCHMARK namespace before running a task."""
import os
import time
import json
import subprocess


def apply_fixtures(fixtures: list):
    for fix in fixtures:
        ftype = fix.get("type")
        if ftype == "cls":
            _apply_cls(fix)
        elif ftype == "global":
            _apply_global(fix)
        elif ftype == "routine":
            _apply_routine(fix)
        else:
            raise ValueError(f"Unknown fixture type: {ftype}")


def _mcp_call(tool: str, args: dict) -> dict:
    from .namespace import _mcp_call as _call
    return _call(tool, args)


def _apply_cls(fix: dict):
    name = fix["name"]
    content = fix["content"]
    _mcp_call("iris_doc", {
        "mode": "put",
        "name": f"{name}.cls",
        "content": content,
        "namespace": "BENCHMARK",
    })
    _mcp_call("iris_compile", {
        "target": f"{name}.cls",
        "namespace": "BENCHMARK",
    })


def _apply_global(fix: dict):
    name = fix["name"]
    subscript = fix.get("subscript", "")
    value = fix["value"]
    sub_str = f'("{subscript}")' if subscript else ""
    code = f'set {name}{sub_str}="{value}"'
    _mcp_call("iris_execute", {
        "code": code,
        "namespace": "BENCHMARK",
        "confirmed": True,
    })


def _apply_routine(fix: dict):
    name = fix["name"]
    content = fix["content"]
    _mcp_call("iris_doc", {
        "mode": "put",
        "name": f"{name}.mac",
        "content": content,
        "namespace": "BENCHMARK",
    })
    _mcp_call("iris_compile", {
        "target": f"{name}.mac",
        "namespace": "BENCHMARK",
    })
