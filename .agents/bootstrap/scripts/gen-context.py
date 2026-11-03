#!/usr/bin/env python3
"""Deterministically compile canonical project sources into agent context projections."""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Compiling context must not leave files behind (an untracked __pycache__ here
# would change the selection a clean checkout computes).
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layout import Layout, TEMPLATES, find_project_root  # noqa: E402

ROOT = find_project_root(Path(__file__).resolve().parent)
LAYOUT = Layout(ROOT)
VERSION = LAYOUT.path("version").read_text(encoding="utf-8").strip()
POLICY_PATH = LAYOUT.path("context_sources")
FALLBACK_POLICY_PATH = ROOT / TEMPLATES / "context-sources.json"
SELF = Path(__file__).resolve().relative_to(ROOT).as_posix()
AGENTS = ROOT / ".agents"
GIT_TIMEOUT_SECONDS = 5
DEFAULT_SENSITIVE_PATTERNS = (
    ".env.*",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "*credentials*.json",
    "*secrets*.json",
)
DEFAULT_EXCLUDED_DIRS = frozenset({
    ".git", "node_modules", "target", "dist", "build", "out", "coverage", "htmlcov",
    "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", ".venv", "venv",
    ".cache", ".next", ".turbo", ".vercel", "tmp", "temp",
})
# Per-machine state that is never a source: the ACP decision log, caches, the policy
# proposal, and upgrade plans and backups (git-ignored, but also skipped outside Git).
LOCAL_STATE_PREFIXES = (".agents/records/local/", ".agents/.upgrades/", ".formal-bootstrap/")
DEFAULT_EXCLUDED_PATHS = frozenset({
    ".agents/llms.txt", ".agents/llms-full.txt", ".agents/context-lock.json",
})
DEFAULT_GENERATED_FILE_PATTERNS = (
    "llms*.txt", "llm-full-docs*.txt", "llm-full-docs-all.zip", "*.tmp", "*.temp",
    "*.bak", "*~", "*.pyc", "*.pyo", "*.tsbuildinfo", "*.log", "*.swp", "*.swo", "*.orig", "*.rej",
)


class ContextError(RuntimeError):
    """A user-actionable context compilation failure."""


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def normalize_path_filters(values: list[str]) -> tuple[str, ...]:
    normalized = []
    for value in values:
        path = Path(value)
        if not value or path.is_absolute() or ".." in path.parts:
            raise ContextError(
                "--path values must be non-empty relative paths inside the project"
            )
        relative = path.as_posix().rstrip("/") or "."
        if not (ROOT / relative).exists():
            raise ContextError(f"--path does not exist: {value}")
        normalized.append(relative)
    return tuple(sorted(set(normalized)))


def run_git(path: Path, args: list[str]) -> str | None:
    try:
        return subprocess.check_output(
            ["git", "-C", str(path), *args],
            text=True,
            stderr=subprocess.DEVNULL,
            timeout=GIT_TIMEOUT_SECONDS,
        ).strip()
    except (OSError, subprocess.SubprocessError):
        return None


def redact_remote(remote: str | None) -> str | None:
    if not remote:
        return remote
    return re.sub(r"(://)([^/@]+)@", r"\1", remote)


def one_repo_state(path: Path, rel: str) -> dict | None:
    commit = run_git(path, ["rev-parse", "HEAD"])
    if not commit:
        return None
    branch = run_git(path, ["branch", "--show-current"]) or None
    status = run_git(path, ["status", "--porcelain"])
    remote = redact_remote(run_git(path, ["remote", "get-url", "origin"]))
    return {
        "path": rel,
        "commit": commit,
        "branch": branch,
        "dirty": bool(status),
        "remote": remote,
    }


def repository_state() -> dict:
    """Capture provenance only; context validity is source-digest based.

    Exact HEAD cannot be a validity predicate for tracked generated files without
    creating a self-reference. We still retain the observed Git state so agents can
    identify the checkout that produced the projection.
    """
    root_state = one_repo_state(ROOT, ".")
    if not root_state:
        return {"available": False, "validationRole": "provenance", "repositories": []}

    repos = [root_state]
    raw = run_git(ROOT, ["submodule", "status", "--recursive"])
    if raw:
        for line in raw.splitlines():
            parts = line.strip().lstrip("-+U").split()
            if len(parts) < 2:
                continue
            rel = parts[1]
            state = one_repo_state(ROOT / rel, rel)
            if state:
                repos.append(state)
            else:
                repos.append(
                    {
                        "path": rel,
                        "commit": parts[0],
                        "branch": None,
                        "dirty": None,
                        "remote": None,
                    }
                )
    return {"available": True, "validationRole": "provenance", "repositories": repos}


def load_policy() -> dict:
    path = POLICY_PATH if POLICY_PATH.exists() else FALLBACK_POLICY_PATH
    try:
        policy = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ContextError(f"cannot load context policy {path}: {exc}") from exc
    if not isinstance(policy, dict):
        raise ContextError("context policy must be a JSON object")
    required = {
        "excludeDirectories": list,
        "excludePaths": list,
        "includeExtensions": list,
        "maxFileBytes": int,
    }
    for key, expected in required.items():
        value = policy.get(key)
        if not isinstance(value, expected):
            raise ContextError(f"context policy field {key!r} must be a {expected.__name__}")
    if policy["maxFileBytes"] <= 0:
        raise ContextError("context policy maxFileBytes must be positive")
    list_fields = (
        "excludeDirectories",
        "excludePaths",
        "includeExtensions",
        "includeFiles",
        "excludePatterns",
    )
    for key in list_fields:
        if key in policy and not isinstance(policy[key], list):
            raise ContextError(f"context policy field {key!r} must be a list")
    return policy


def candidate_paths() -> list[Path]:
    """Every file Git tracks or would add, so ignored local files never reach the corpus.

    A context lock has to match what a clean checkout of the same commit
    computes. Outside a Git work tree every file is a candidate.
    """
    listed = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        capture_output=True,
    )
    if listed.returncode != 0:
        return sorted(ROOT.rglob("*"))
    names = {name for name in listed.stdout.decode("utf-8", "surrogateescape").split("\0") if name}
    return [ROOT / name for name in sorted(names)]


def tracked_paths() -> set[str] | None:
    """What Git tracks, or None outside a work tree. Exclusions are counted only among these,
    so an untracked local file (a cache, a scratch note) never changes the recorded selection."""
    listed = subprocess.run(["git", "-C", str(ROOT), "ls-files", "-z", "--cached"], capture_output=True)
    if listed.returncode != 0:
        return None
    return {name for name in listed.stdout.decode("utf-8", "surrogateescape").split("\0") if name}


# Index entry modes that carry file text. Symlinks (120000) and submodule
# gitlinks (160000) are not file contents.
BLOB_MODES = {"100644", "100755"}
STAGED_SYMLINKS: list[str] = []      # filled by staged_entries(); counted like working-tree symlinks


def git_bytes(args: list[str], data: bytes | None = None) -> bytes:
    try:
        return subprocess.run(["git", "-C", str(ROOT), *args], input=data,
                              capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise ContextError(f"--staged needs a Git work tree: git {args[0]} failed") from exc


def staged_entries() -> dict[str, str]:
    """Path -> blob id for every regular file in the index.

    `--staged` reads membership and content from the index, so the lock written
    for a commit is the one a clean checkout of that commit computes, whatever
    else is uncommitted in the working tree.
    """
    entries: dict[str, str] = {}
    for record in git_bytes(["ls-files", "-s", "-z", "--cached"]).decode("utf-8", "surrogateescape").split("\0"):
        if not record:
            continue
        meta, _, rel = record.partition("\t")
        mode, blob, stage = (meta.split() + ["", "", ""])[:3]
        if stage != "0":
            raise ContextError(f"the index has unresolved conflicts at {rel}; resolve them before --staged")
        if mode in BLOB_MODES:
            entries[rel] = blob
        elif mode == "120000":
            STAGED_SYMLINKS.append(rel)
    return entries


def blob_sizes(blobs: list[str]) -> list[int]:
    if not blobs:
        return []
    out = git_bytes(["cat-file", "--batch-check=%(objectsize)"], ("\n".join(blobs) + "\n").encode())
    return [int(line) if line.strip().isdigit() else -1 for line in out.decode().splitlines()]


def read_blobs(blobs: list[str]) -> list[bytes]:
    if not blobs:
        return []
    data = git_bytes(["cat-file", "--batch"], ("\n".join(blobs) + "\n").encode())
    result, pos = [], 0
    for blob in blobs:
        end = data.index(b"\n", pos)
        header = data[pos:end].decode().split()
        pos = end + 1
        if len(header) < 3 or header[1] != "blob":
            raise ContextError(f"staged blob {blob} is missing from the object store")
        size = int(header[2])
        result.append(data[pos:pos + size])
        pos += size + 1
    return result


def staged_version() -> str:
    blob = staged_entries().get(LAYOUT.rel("version"))
    return read_blobs([blob])[0].decode("utf-8").strip() if blob else VERSION


def source_files(
    policy: dict, path_filters: tuple[str, ...] = (), staged: bool = False
) -> tuple[list[tuple[str, bytes]], dict[str, int]]:
    ex_dirs = set(policy["excludeDirectories"]) | DEFAULT_EXCLUDED_DIRS
    ex_paths = set(policy["excludePaths"]) | DEFAULT_EXCLUDED_PATHS
    exts = set(policy["includeExtensions"])
    include_files = set(policy.get("includeFiles", []))
    max_bytes = int(policy["maxFileBytes"])
    max_total_bytes = policy.get("maxTotalBytes")
    if max_total_bytes is not None and (
        not isinstance(max_total_bytes, int) or max_total_bytes <= 0
    ):
        raise ContextError("context policy maxTotalBytes must be a positive integer")
    include_vendor = bool(policy.get("includeVendor", True))
    sensitive_patterns = tuple(
        DEFAULT_SENSITIVE_PATTERNS + tuple(policy.get("excludePatterns", []))
    )
    result: list[tuple[str, bytes]] = []
    skipped: dict[str, int] = {}
    total_bytes = 0

    tracked = None if staged else tracked_paths()

    def skip(reason: str, rel: str) -> None:
        if "__pycache__" in rel.split("/") or rel.endswith((".pyc", ".pyo")):
            return                     # interpreter caches are never sources and vary by machine
        if tracked is None or rel in tracked:
            skipped[reason] = skipped.get(reason, 0) + 1

    index = staged_entries() if staged else {}
    for rel in STAGED_SYMLINKS if staged else ():
        skip("symlink", rel)
    sizes = dict(zip(index, blob_sizes(list(index.values())))) if staged else {}
    pending: list[tuple[str, str]] = []
    for path in ([ROOT / rel for rel in sorted(index)] if staged else candidate_paths()):
        if not staged and path.is_symlink():
            skip("symlink", path.relative_to(ROOT).as_posix())
            continue
        if not staged and not path.is_file():
            continue
        rel = path.relative_to(ROOT).as_posix()
        parts = rel.split("/")
        if parts[0] == ".git":
            continue
        if path_filters and not any(
            rel == selected or rel.startswith(selected + "/") for selected in path_filters
        ):
            skip("path-filter", rel)
            continue
        if any(part in ex_dirs for part in parts[:-1]):
            skip("excluded-directory", rel)
            continue
        if not include_vendor and parts[0] == "vendor":
            skip("vendor", rel)
            continue
        if rel in ex_paths or rel.startswith(LOCAL_STATE_PREFIXES):
            continue
        if any(fnmatch.fnmatch(path.name, pattern) for pattern in DEFAULT_GENERATED_FILE_PATTERNS):
            skip("generated-or-transient-file", rel)
            continue
        if any(fnmatch.fnmatch(rel, pattern) for pattern in sensitive_patterns):
            skip("excluded-path", rel)
            continue
        if rel not in include_files and path.suffix.lower() not in exts:
            skip("extension", rel)
            continue
        if staged:
            if sizes[rel] > max_bytes:
                skip("size", rel)
            else:
                pending.append((rel, index[rel]))
            continue
        try:
            before = path.stat()
        except OSError as exc:
            raise ContextError(f"cannot stat eligible source {rel}: {exc}") from exc
        if before.st_size > max_bytes:
            skip("size", rel)
            continue
        try:
            data = path.read_bytes()
            data.decode("utf-8")
            after = path.stat()
        except (OSError, UnicodeDecodeError) as exc:
            raise ContextError(f"cannot read eligible source {rel}: {exc}") from exc
        if (before.st_size, before.st_mtime_ns, before.st_ino) != (
            after.st_size,
            after.st_mtime_ns,
            after.st_ino,
        ):
            raise ContextError(f"source changed during compilation: {rel}")
        if max_total_bytes is not None and total_bytes + len(data) > max_total_bytes:
            raise ContextError(f"context source total exceeds maxTotalBytes at {rel}")
        result.append((rel, data))
        total_bytes += len(data)

    for (rel, _), data in zip(pending, read_blobs([blob for _, blob in pending])):
        try:
            data.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ContextError(f"cannot read eligible source {rel}: {exc}") from exc
        if max_total_bytes is not None and total_bytes + len(data) > max_total_bytes:
            raise ContextError(f"context source total exceeds maxTotalBytes at {rel}")
        result.append((rel, data))
        total_bytes += len(data)

    if skipped:
        print(
            "excluded " + ", ".join(f"{key}={value}" for key, value in sorted(skipped.items())),
            file=sys.stderr,
        )
    return sorted(result, key=lambda item: item[0]), skipped


def build(path_filters: tuple[str, ...] = (), staged: bool = False) -> tuple[dict, str, str]:
    policy = load_policy()
    sources, skipped = source_files(policy, path_filters, staged)
    version = staged_version() if staged else VERSION
    entries = [
        {"path": rel, "bytes": len(data), "sha256": sha(data)} for rel, data in sources
    ]
    aggregate = sha(
        "\n".join(f"{entry['path']}:{entry['sha256']}" for entry in entries).encode()
    )
    repos = repository_state()
    lock = {
        "format": "2",
        "generatorVersion": version,
        "root": ".",
        "validityBasis": "aggregate-source-digest",
        "selectedPaths": list(path_filters),
        "sourceSelection": {
            "includedCount": len(sources),
            "includedBytes": sum(entry["bytes"] for entry in entries),
            "excludedCounts": dict(sorted(skipped.items())),
        },
        "repositoryState": repos,
        "aggregateSourceDigest": aggregate,
        "sources": entries,
    }

    index = [
        "# Project Context Index",
        f"> Generator: {version}",
        f"> Source digest: `{aggregate}`",
        "",
        "> Generated projection. Canonical sources remain authoritative.",
        "",
    ]
    full = [
        "# PROJECT COMPILED CONTEXT",
        f"Generator: {version}",
        f"Source digest: {aggregate}",
        "",
        "> Generated projection. Canonical sources remain authoritative.",
        "",
    ]

    for rel, data in sources:
        digest = sha(data)
        index.append(f"- `{rel}` | {len(data)} bytes | `{digest}`")
        full.extend(["", "=" * 72, f"FILE: {rel}", "=" * 72, "", data.decode("utf-8")])

    return lock, "\n".join(index) + "\n", "\n".join(full) + "\n"


HOOK_MARKER = "# formal-project-bootstrap: compile context from the staged tree"
HOOK = f"""#!/bin/sh
{HOOK_MARKER}
# Installed by {SELF} --install-hook. Bypass: SKIP_CONTEXT_HOOK=1 git commit ...
[ "${{SKIP_CONTEXT_HOOK:-0}}" = "1" ] && exit 0
root=$(git rev-parse --show-toplevel) || exit 1
cd "$root" || exit 1
[ -f {SELF} ] || exit 0
git rev-parse -q --verify MERGE_HEAD >/dev/null 2>&1 && exit 0
python3 {SELF} --staged >/dev/null || {{
    echo "pre-commit: gen-context.py --staged failed; commit aborted (bypass: SKIP_CONTEXT_HOOK=1)." >&2
    exit 1
}}
for generated in .agents/context-lock.json .agents/llms.txt .agents/llms-full.txt; do
    if git ls-files --error-unmatch "$generated" >/dev/null 2>&1 || [ "$generated" = .agents/context-lock.json ]; then
        git add -- "$generated"
    fi
done
"""


def install_hook() -> int:
    """Write the pre-commit hook into this repository's hooks directory."""
    hooks = run_git(ROOT, ["rev-parse", "--git-path", "hooks"])
    if not hooks:
        print("ERROR: --install-hook needs a Git work tree", file=sys.stderr)
        return 2
    directory = (Path(hooks) if Path(hooks).is_absolute() else ROOT / hooks).resolve()
    common = run_git(ROOT, ["rev-parse", "--git-common-dir"]) or ".git"
    common_dir = (Path(common) if Path(common).is_absolute() else ROOT / common).resolve()
    if not (directory.is_relative_to(ROOT.resolve()) or directory.is_relative_to(common_dir)):
        print(f"ERROR: core.hooksPath points outside this repository ({directory}); a hook there would "
              f"run in every repository that shares it. Add `python3 {SELF} --staged` and "
              "`git add .agents/context-lock.json` to that hook yourself.", file=sys.stderr)
        return 2
    hook = directory / "pre-commit"
    if hook.exists() and HOOK_MARKER not in hook.read_text(encoding="utf-8", errors="replace"):
        print(f"ERROR: {hook} already exists and is not this hook; add `python3 {SELF} "
              "--staged` and `git add .agents/context-lock.json` to it instead", file=sys.stderr)
        return 2
    hook.parent.mkdir(parents=True, exist_ok=True)
    hook.write_text(HOOK, encoding="utf-8")
    hook.chmod(0o755)
    print(f"installed {hook}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--staged",
        action="store_true",
        help="read sources from the Git index, so the lock matches the commit being made",
    )
    parser.add_argument(
        "--path",
        action="append",
        dest="path_filters",
        default=[],
        help="compile only this relative file or directory (repeatable)",
    )
    parser.add_argument("--install-hook", action="store_true",
                        help="install a pre-commit hook that runs --staged and stages the lock")
    args = parser.parse_args()
    if args.install_hook:
        return install_hook()
    try:
        path_filters = normalize_path_filters(args.path_filters)
        lock, index, full = build(path_filters, args.staged)
    except ContextError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if args.check:
        # With --staged the comparison is against the staged lock: what the commit would carry.
        def committed(name: str) -> bytes | None:
            path = AGENTS / name
            if not args.staged:
                return path.read_bytes() if path.exists() else None
            shown = subprocess.run(["git", "-C", str(ROOT), "show", f":{path.relative_to(ROOT).as_posix()}"],
                                   capture_output=True)
            return shown.stdout if shown.returncode == 0 else None

        where = "staged " if args.staged else ""
        raw_lock = committed("context-lock.json")
        if raw_lock is None:
            print(f"STALE: {where}context lock missing", file=sys.stderr)
            return 2
        try:
            current = json.loads(raw_lock.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            print(f"STALE: {where}context lock is unreadable: {exc}", file=sys.stderr)
            return 2
        if current.get("aggregateSourceDigest") != lock["aggregateSourceDigest"]:
            print(f"STALE: {where}source digest differs", file=sys.stderr)
            return 2
        if current.get("generatorVersion") != lock["generatorVersion"]:
            print(f"STALE: {where}generator version differs", file=sys.stderr)
            return 2
        for key in ("format", "root", "validityBasis", "selectedPaths", "sourceSelection", "sources"):
            if current.get(key) != lock[key]:
                print(f"STALE: {where}context lock field differs: {key}", file=sys.stderr)
                return 2
        for name, expected in (("llms.txt", index), ("llms-full.txt", full)):
            output = committed(name)
            if output is not None and output != expected.encode("utf-8"):
                print(f"STALE: {where}{name} differs", file=sys.stderr)
                return 2
        print(f"OK {lock['aggregateSourceDigest']} (generated exports absent or current)")
        return 0

    AGENTS.mkdir(parents=True, exist_ok=True)
    outputs = {
        "context-lock.json": json.dumps(lock, indent=2) + "\n",
        "llms.txt": index,
        "llms-full.txt": full,
    }
    try:
        with tempfile.TemporaryDirectory(dir=AGENTS.parent) as tmp:
            temp_root = Path(tmp)
            for name, content in outputs.items():
                temp_path = temp_root / name
                temp_path.write_bytes(content.encode("utf-8"))
            for name in outputs:
                os.replace(temp_root / name, AGENTS / name)
    except OSError as exc:
        print(f"ERROR: unable to publish generated context atomically: {exc}", file=sys.stderr)
        return 2
    print(f"generated {len(lock['sources'])} sources {lock['aggregateSourceDigest']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
