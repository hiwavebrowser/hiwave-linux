"""
aiorch.py - local-first AI orchestration utilities.

Design goals:
- Zero third-party dependencies (stdlib only) so it runs on a fresh Windows box.
- Schemas live in tools/ai-orchestrator/schemas/ as JSON Schema, but validation
  here is intentionally lightweight and pragmatic.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Tuple


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
AI_DIR = REPO_ROOT / ".ai"
SCHEMAS_DIR = pathlib.Path(__file__).resolve().parent / "schemas"


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def read_json(path: pathlib.Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def die(msg: str) -> None:
    print(f"[aiorch] ERROR: {msg}", file=sys.stderr)
    raise SystemExit(2)


def ensure_dirs() -> None:
    (AI_DIR / "work_orders").mkdir(parents=True, exist_ok=True)
    (AI_DIR / "reports").mkdir(parents=True, exist_ok=True)
    (AI_DIR / "artifacts").mkdir(parents=True, exist_ok=True)
    (AI_DIR / "cache").mkdir(parents=True, exist_ok=True)


def _require(obj: Dict[str, Any], key: str, expected_type: Any) -> Any:
    if key not in obj:
        die(f"Missing required key '{key}'")
    val = obj[key]
    if expected_type is not Any and not isinstance(val, expected_type):
        die(f"Key '{key}' must be {expected_type}, got {type(val)}")
    return val


def _require_enum(obj: Dict[str, Any], key: str, allowed: List[str]) -> str:
    val = _require(obj, key, str)
    if val not in allowed:
        die(f"Key '{key}' must be one of {allowed}, got '{val}'")
    return val


def validate_roadmap_index(doc: Dict[str, Any]) -> None:
    _require(doc, "schema_version", str)
    _require(doc, "created_at", str)
    _require(doc, "updated_at", str)
    work_orders = _require(doc, "work_orders", list)

    ids: set[str] = set()
    for i, wo in enumerate(work_orders):
        if not isinstance(wo, dict):
            die(f"work_orders[{i}] must be an object")
        wo_id = _require(wo, "id", str)
        if wo_id in ids:
            die(f"Duplicate work order id: {wo_id}")
        ids.add(wo_id)
        _require(wo, "title", str)
        _require_enum(wo, "status", ["pending", "in_progress", "completed", "blocked", "cancelled"])
        deps = wo.get("depends_on", [])
        if deps is None:
            deps = []
        if not isinstance(deps, list) or any(not isinstance(d, str) for d in deps):
            die(f"work_orders[{i}].depends_on must be a list of strings")

    # Validate dependency references.
    for wo in work_orders:
        for dep in wo.get("depends_on", []) or []:
            if dep not in ids:
                die(f"Work order '{wo['id']}' depends_on unknown id '{dep}'")


def cmd_validate_roadmap(_: argparse.Namespace) -> None:
    ensure_dirs()
    roadmap_path = AI_DIR / "roadmap_index.json"
    if not roadmap_path.exists():
        die(f"Missing {roadmap_path}. Run init (or create it) first.")
    doc = read_json(roadmap_path)
    validate_roadmap_index(doc)
    print(f"[aiorch] OK: {roadmap_path} is structurally valid.")


def cmd_init(_: argparse.Namespace) -> None:
    ensure_dirs()
    roadmap_path = AI_DIR / "roadmap_index.json"
    if roadmap_path.exists():
        print(f"[aiorch] {roadmap_path} already exists; leaving as-is.")
        return

    doc = {
        "schema_version": "roadmap_index.v1",
        "created_at": utc_now_iso(),
        "updated_at": utc_now_iso(),
        "work_orders": [],
    }
    roadmap_path.write_text(json.dumps(doc, indent=2), encoding="utf-8")
    print(f"[aiorch] Created {roadmap_path}")


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="aiorch", description="Local-first AI orchestration utilities")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_init = sub.add_parser("init", help="Create repo-local .ai/ directories and a skeleton roadmap index.")
    p_init.set_defaults(func=cmd_init)

    p_val = sub.add_parser("validate-roadmap", help="Validate .ai/roadmap_index.json (lightweight).")
    p_val.set_defaults(func=cmd_validate_roadmap)

    args = parser.parse_args(argv)
    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
