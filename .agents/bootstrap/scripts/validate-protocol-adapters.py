#!/usr/bin/env python3
"""Validate every bundled protocol adapter without external dependencies."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layout import Layout, find_project_root  # noqa: E402

ROOT = find_project_root(Path(__file__).resolve().parent)
LAYOUT = Layout(ROOT)
CURRENT_BOOTSTRAP = LAYOUT.path("version").read_text(encoding="utf-8").strip()
REQUIRED = {"format", "adapterId", "protocol", "supportedBootstrap", "authority", "mappings", "unsupported", "compatibility", "checks", "migration"}
DIMENSIONS = {"syntax", "schema", "behavior", "semantics", "authority", "evidence"}


def main() -> int:
    schema = LAYOUT.schema("protocol-adapter.schema.json")
    if not schema.exists():
        print(f"INVALID: missing {schema}", file=sys.stderr)
        return 2
    adapters = sorted(LAYOUT.path("protocol_profiles").glob("*/adapter.json"))
    if LAYOUT.path("installation").exists():
        # A project validates the adapters it selected; the bootstrap repository validates every bundled one.
        try:
            profile = json.loads(LAYOUT.path("profile").read_text(encoding="utf-8"))
        except (OSError, ValueError):
            profile = {}
        selected = set(profile.get("protocolAdapters") or profile.get("profiles") or [])
        adapters = [path for path in adapters if path.parent.name in selected]
    if not adapters:
        print("OK protocol adapters: none selected")
        return 0
    errors: list[str] = []
    ids: set[str] = set()
    for path in adapters:
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            errors.append(f"{path}: {exc}")
            continue
        missing = REQUIRED - set(value)
        if missing:
            errors.append(f"{path}: missing {sorted(missing)}")
        ident = value.get("adapterId")
        if ident in ids:
            errors.append(f"duplicate adapter id: {ident}")
        ids.add(ident)
        protocol = value.get("protocol", {})
        for field in ("id", "version", "revisionOrDigest", "canonicalSource"):
            if not isinstance(protocol.get(field), str) or not protocol[field]:
                errors.append(f"{path}: protocol.{field} is required")
        compatibility = value.get("compatibility", {})
        if set(compatibility) != DIMENSIONS:
            errors.append(f"{path}: compatibility must contain exactly {sorted(DIMENSIONS)}")
        for section in ("imports", "projects", "lifecycle", "evidence"):
            if not isinstance(value.get("mappings", {}).get(section), list):
                errors.append(f"{path}: mappings.{section} must be an array")
        supported = value.get("supportedBootstrap")
        if supported not in {CURRENT_BOOTSTRAP, f"{CURRENT_BOOTSTRAP.rsplit('.', 1)[0]}.x"}:
            errors.append(f"{path}: supportedBootstrap {supported!r} does not cover FPB {CURRENT_BOOTSTRAP}")
    if errors:
        print("INVALID PROTOCOL ADAPTERS", file=sys.stderr)
        print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
        return 1
    print(f"OK protocol adapters: {len(adapters)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
