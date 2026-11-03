#!/usr/bin/env python3
"""Validate the local E0/E1 work ledger; it never authorizes remote effects."""
from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layout import Layout, find_project_root  # noqa: E402

ROOT = find_project_root(Path(__file__).resolve().parent)
LAYOUT = Layout(ROOT)
LEDGER = LAYOUT.record("local-work-items.jsonl")
MAX_LEDGER_BYTES = 4 * 1024 * 1024
MAX_RECORD_BYTES = 64 * 1024
ID = re.compile(r"^[a-z0-9][a-z0-9._:-]{0,127}$")
STATES = {"Backlog", "Ready", "In progress", "In review", "Done", "Verified", "Blocked"}
LOCAL_PERMISSIONS = {"repository.read", "source.inspect", "local.analysis", "local.test", "local.edit", "local.commit"}
E0_PERMISSIONS = LOCAL_PERMISSIONS - {"local.edit", "local.commit"}
REQUIRED = {"id", "state", "intentRef", "objectiveOrHypothesis", "scope", "exclusions", "dependencies",
            "acceptance", "evidenceRequirements", "permissions", "effectClass", "target", "budget", "stopConditions"}
OPTIONAL = {"parentRef", "evaluatorRef", "completedAt", "completionEvidence"}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate object key")
        value[key] = item
    return value


def nonempty_strings(value: object) -> bool:
    return isinstance(value, list) and bool(value) and all(isinstance(item, str) and item.strip() for item in value)


def validate(item: object) -> str | None:
    if not isinstance(item, dict) or set(item) - REQUIRED - OPTIONAL:
        return "record has an invalid shape"
    if REQUIRED - set(item):
        return "record is missing required Ready fields"
    if not isinstance(item["id"], str) or not ID.fullmatch(item["id"]):
        return "id is invalid"
    if not isinstance(item["state"], str) or item["state"] not in STATES:
        return "state is invalid"
    for key in ("intentRef", "objectiveOrHypothesis"):
        if not isinstance(item[key], str) or not item[key].strip():
            return f"{key} must be non-empty"
    for key in ("scope", "exclusions", "acceptance", "evidenceRequirements", "stopConditions"):
        if not nonempty_strings(item[key]):
            return f"{key} must be a non-empty string array"
    if not isinstance(item["dependencies"], list) or not all(isinstance(value, str) and value for value in item["dependencies"]):
        return "dependencies must be a string array"
    if not isinstance(item["effectClass"], str) or item["effectClass"] not in {"E0", "E1", "E2", "E3", "E4"}:
        return "effectClass is invalid"
    if not isinstance(item["permissions"], list) or not item["permissions"] or not all(isinstance(value, str) for value in item["permissions"]):
        return "permissions must be a non-empty string array"
    if item["effectClass"] in {"E0", "E1"}:
        if not set(item["permissions"]) <= LOCAL_PERMISSIONS:
            return "active local work permissions must use only the E0/E1 local vocabulary"
        if item["effectClass"] == "E0" and not set(item["permissions"]) <= E0_PERMISSIONS:
            return "E0 work cannot include edit or local commit permissions"
    elif (item["state"] not in {"Done", "Verified"} or not isinstance(item.get("completedAt"), str)
          or not item["completedAt"].strip() or not isinstance(item.get("completionEvidence"), dict)
          or not item["completionEvidence"]):
        return "E2+ records are accepted only as completed historical evidence; they authorize no current effect"
    target = item["target"]
    if not isinstance(target, dict) or set(target) - {"repositoryOrArtifact", "authorizedPaths"}:
        return "target has an invalid shape"
    if not isinstance(target.get("repositoryOrArtifact"), str) or not target["repositoryOrArtifact"].strip():
        return "target.repositoryOrArtifact must be non-empty"
    paths = target.get("authorizedPaths", [])
    if not isinstance(paths, list) or not all(isinstance(path, str) and path and not path.startswith("/") for path in paths):
        return "target.authorizedPaths must contain relative paths"
    budget = item["budget"]
    if not isinstance(budget, dict) or set(budget) - {"retries", "money", "humanAttentionMinutes"} or not budget:
        return "budget has an invalid shape"
    if any(type(value) not in (int, float) or not math.isfinite(value) or value < 0 for value in budget.values()):
        return "budget values must be non-negative numbers"
    for key in ("parentRef", "evaluatorRef"):
        if key in item and (not isinstance(item[key], str) or not item[key].strip()):
            return f"{key} must be non-empty when present"
    return None


def main() -> int:
    try:
        if LEDGER.is_symlink() or not LEDGER.is_file() or LEDGER.stat().st_size > MAX_LEDGER_BYTES:
            raise ValueError("ledger is missing, unsafe, or exceeds 4 MiB")
        raw = LEDGER.read_bytes()
        text = raw.decode("utf-8")
        seen: set[str] = set()
        count = 0
        for line_number, line in enumerate(text.splitlines(), 1):
            if not line.strip():
                continue
            if len(line.encode("utf-8")) > MAX_RECORD_BYTES:
                raise ValueError(f"line {line_number} exceeds 64 KiB")
            item = json.loads(line, object_pairs_hook=unique_object)
            problem = validate(item)
            if problem:
                raise ValueError(f"line {line_number}: {problem}")
            if item["id"] in seen:
                raise ValueError(f"line {line_number}: duplicate id {item['id']}")
            seen.add(item["id"])
            count += 1
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        print(f"INVALID: {exc}", file=sys.stderr)
        return 2
    print(f"VALID: {count} local work item(s); ledger grants no remote or governed authority")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
