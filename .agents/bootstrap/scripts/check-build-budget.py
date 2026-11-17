#!/usr/bin/env python3
"""Audit what a repository can make paid build services and the local machine do.

  python3 .agents/bootstrap/scripts/check-build-budget.py [--root DIR] [--strict]

FAIL (exit 1):
  - a workflow that runs on pushes to any branch (no `branches:` filter under `push:`)
  - a workflow job without `timeout-minutes`
WARN (exit 1 with --strict):
  - a workflow without a `concurrency` group, a scheduled run, or a matrix
  - a vercel.json that neither sets `ignoreCommand` nor turns branches off with `git.deploymentEnabled`
    (if the repository is connected to Vercel, every push to every branch builds)
  - a Rust package with more than one file in tests/ (each links its own program against every dependency)
See MULTI_AGENT_COORDINATION.md, "Budgets". Standard library only.
"""
import argparse
import json
import os
import re
import sys

PRUNE = {"node_modules", ".git", "target", "dist", "build", ".next", ".turbo", ".venv", "vendor"}


def walk(root, max_depth):
    for dirpath, dirnames, filenames in os.walk(root):
        depth = dirpath[len(root):].count(os.sep)
        dirnames[:] = [] if depth >= max_depth else sorted(d for d in dirnames if d not in PRUNE)
        yield dirpath, filenames


def on_block(text):
    inline = re.search(r"(?m)^(?:on|\"on\"):[ \t]*(\[[^\]]*\]|\w+)[ \t]*$", text)
    if inline:
        return None, inline.group(1)
    block = re.search(r"(?ms)^(?:on|\"on\"):[ \t]*\n(.*?)(?=^\S)", text + "\nEND")
    return (block.group(1) if block else ""), None


def push_any_branch(text):
    block, inline = on_block(text)
    if inline is not None:
        return "push" in inline
    push = re.search(r"(?m)^([ \t]+)push:[ \t]*(.*)$", block)
    if not push or push.group(2).strip() not in ("", "{}", "null", "~"):
        return False
    child = re.match(r"\n([ \t]+)(\w[\w-]*):", block[push.end():])
    return not child or len(child.group(1)) <= len(push.group(1)) or child.group(2) not in ("branches", "tags")


def jobs_without_timeout(text):
    body = re.search(r"(?ms)^jobs:[ \t]*\n(.*)", text)
    if not body:
        return []
    jobs = list(re.finditer(r"(?m)^  ([A-Za-z0-9_-]+):[ \t]*$", body.group(1)))
    missing = []
    for i, job in enumerate(jobs):
        end = jobs[i + 1].start() if i + 1 < len(jobs) else len(body.group(1))
        chunk = body.group(1)[job.end():end]
        if "timeout-minutes:" not in chunk and "uses:" not in re.split(r"(?m)^    steps:", chunk)[0]:
            missing.append(job.group(1))
    return missing


def audit(root):
    fails, warns = [], []
    wf_dir = os.path.join(root, ".github", "workflows")
    for name in sorted(os.listdir(wf_dir)) if os.path.isdir(wf_dir) else []:
        if not name.endswith((".yml", ".yaml")):
            continue
        text = open(os.path.join(wf_dir, name), errors="ignore").read()
        rel = f".github/workflows/{name}"
        if push_any_branch(text):
            fails.append(f"{rel}: runs on pushes to any branch; limit `push:` to main (`branches: [main]`)")
        missing = jobs_without_timeout(text)
        if missing:
            fails.append(f"{rel}: jobs without timeout-minutes: {', '.join(missing)}")
        if not re.search(r"(?m)^\s*concurrency:", text):
            warns.append(f"{rel}: no concurrency group; a newer push won't cancel the run in progress")
        crons = re.findall(r"cron:\s*['\"]([^'\"]+)", text)
        if crons:
            warns.append(f"{rel}: scheduled runs ({', '.join(crons)}) spend minutes with no change to test")
        if "matrix:" in text:
            warns.append(f"{rel}: matrix multiplies every run; keep it only where it's asked for")
    for dirpath, filenames in walk(root, 3):
        if "vercel.json" in filenames:
            rel = os.path.relpath(os.path.join(dirpath, "vercel.json"), root)
            try:
                cfg = json.load(open(os.path.join(dirpath, "vercel.json")))
            except (OSError, ValueError):
                warns.append(f"{rel}: unreadable")
                continue
            enabled = cfg.get("git", {}).get("deploymentEnabled", True)
            stops = enabled is False or (isinstance(enabled, dict) and any(v is False for v in enabled.values()))
            if "ignoreCommand" not in cfg and not stops:
                warns.append(f"{rel}: no ignoreCommand or git.deploymentEnabled; if the repo is connected to"
                             " Vercel, every push to every branch builds")
        if "Cargo.toml" in filenames and os.path.isdir(os.path.join(dirpath, "tests")):
            tests = [f for f in os.listdir(os.path.join(dirpath, "tests")) if f.endswith(".rs")]
            if len(tests) > 1:
                rel = os.path.relpath(dirpath, root)
                warns.append(f"{rel}/tests: {len(tests)} integration test files link {len(tests)} programs;"
                             " one test program per package (modules under tests/<name>/) is the default")
    return fails, warns


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    fails, warns = audit(os.path.abspath(args.root))
    for line in fails:
        print(f"FAIL {line}")
    for line in warns:
        print(f"WARN {line}")
    if not fails and not warns:
        print("OK build budget")
    return 1 if fails or (args.strict and warns) else 0


if __name__ == "__main__":
    sys.exit(main())
