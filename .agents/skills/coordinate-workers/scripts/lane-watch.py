#!/usr/bin/env python3
"""Watch lane state files and print one line per new question or newly blocked lane.

Usage:
  lane-watch.py STATE_DIR              watch until interrupted (poll the local files)
  lane-watch.py STATE_DIR --once       scan once, print, exit
  lane-watch.py STATE_DIR --init LANE [LANE ...]   create empty lane folders

STATE_DIR holds one folder per lane with state.json (the lane writes) and
inbox.jsonl (the coordinator writes). Output lines:

  QUESTION <lane> <id>: <text>
  BLOCKED <lane> on <ticket>: <note>

A question is reported once, and not at all if the inbox already answers it
(a line with "re" equal to its id). A blocked lane is reported once per
distinct "updated" value (the note when "updated" is null). The seen-set is
kept in STATE_DIR/.coordinator-seen.json. Standard library only; it makes no
network calls.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

SEEN_NAME = ".coordinator-seen.json"
EMPTY_STATE = {"updated": None, "ticket": None, "status": "unknown", "prs": [],
               "note": "", "inbox_seen": None, "questions": []}


def load_seen(state_dir: Path) -> dict:
    try:
        data = json.loads((state_dir / SEEN_NAME).read_text(encoding="utf-8"))
        return {"questions": list(data.get("questions", [])), "blocked": dict(data.get("blocked", {}))}
    except (OSError, ValueError, AttributeError):
        return {"questions": [], "blocked": {}}


def answered_ids(inbox: Path) -> set:
    ids = set()
    try:
        lines = inbox.read_text(encoding="utf-8").splitlines()
    except OSError:
        return ids
    for line in lines:
        try:
            entry = json.loads(line)
        except ValueError:
            continue
        if isinstance(entry, dict) and entry.get("re"):
            ids.add(entry["re"])
    return ids


def scan(state_dir: Path, seen: dict) -> list[str]:
    """Return new report lines and update `seen` in place."""
    out: list[str] = []
    for state_file in sorted(state_dir.glob("*/state.json")):
        lane = state_file.parent.name
        try:
            state = json.loads(state_file.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue  # a lane may be mid-write; the next scan reads it
        if not isinstance(state, dict):
            continue
        answered = answered_ids(state_file.parent / "inbox.jsonl")
        for question in state.get("questions") or []:
            qid = question.get("id") if isinstance(question, dict) else None
            if qid and qid not in seen["questions"] and qid not in answered:
                seen["questions"].append(qid)
                out.append(f"QUESTION {lane} {qid}: {str(question.get('text', ''))[:300]}")
        if state.get("status") == "blocked":
            marker = state.get("updated") or state.get("note") or "blocked"
            if seen["blocked"].get(lane) != marker:
                seen["blocked"][lane] = marker
                out.append(f"BLOCKED {lane} on {state.get('ticket')}: {str(state.get('note', ''))[:200]}")
        else:
            seen["blocked"].pop(lane, None)  # a later block is a new report
    return out


def save_seen(state_dir: Path, seen: dict) -> None:
    tmp = state_dir / (SEEN_NAME + ".tmp")
    tmp.write_text(json.dumps(seen), encoding="utf-8")
    tmp.replace(state_dir / SEEN_NAME)


def init_lanes(state_dir: Path, lanes: list[str]) -> None:
    for lane in lanes:
        folder = state_dir / lane
        folder.mkdir(parents=True, exist_ok=True)
        state = folder / "state.json"
        if not state.exists():
            state.write_text(json.dumps({"lane": lane, **EMPTY_STATE}, indent=2) + "\n", encoding="utf-8")
        (folder / "inbox.jsonl").touch()
        print(f"lane {lane}: {folder}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("state_dir", type=Path)
    parser.add_argument("--once", action="store_true", help="scan once and exit")
    parser.add_argument("--interval", type=float, default=20.0, help="seconds between scans (default 20)")
    parser.add_argument("--init", nargs="+", metavar="LANE", help="create empty lane folders and exit")
    args = parser.parse_args(argv)
    state_dir = args.state_dir.expanduser()
    if args.init:
        init_lanes(state_dir, args.init)
        return 0
    if not state_dir.is_dir():
        print(f"no such state folder: {state_dir}", file=sys.stderr)
        return 2
    seen = load_seen(state_dir)
    while True:
        for line in scan(state_dir, seen):
            print(line, flush=True)
        save_seen(state_dir, seen)
        if args.once:
            return 0
        time.sleep(args.interval)


if __name__ == "__main__":
    raise SystemExit(main())
