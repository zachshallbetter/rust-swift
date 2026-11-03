"""The `bootstrap` command: validate, context, upgrade, repair, uninstall, reconcile, version."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

from layout import (
    RELEASE_MANIFEST, SHIPPED_SKILLS, Layout, contained_path, find_project_root, release_machinery,
)

HERE = Path(__file__).resolve().parent
DEFAULT_SOURCE = "https://github.com/zachshallbetter/formal-project-bootstrap"
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "formal-project-bootstrap"
TAG = re.compile(r"\Av(\d+)\.(\d+)\.(\d+)\Z")


def project_root(args) -> Path:
    return Path(args.project).resolve() if getattr(args, "project", None) else find_project_root()


def script(name: str, *argv: str, cwd: Path) -> int:
    return subprocess.call([sys.executable, str(HERE / name), *argv], cwd=cwd)


def installation(project: Path) -> dict:
    try:
        return json.loads(Layout(project).path("installation").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def git(project: Path, *argv: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(project), *argv], capture_output=True, text=True)


# ------------------------------------------------------------------ releases

def latest_tag(source: str) -> str:
    out = subprocess.run(["git", "ls-remote", "--tags", "--refs", source], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"cannot list releases at {source}: {out.stderr.strip()[:200]}")
    tags = [line.rsplit("/", 1)[-1] for line in out.stdout.splitlines()]
    tags = [tag for tag in tags if TAG.match(tag)]
    if not tags:
        raise SystemExit(f"no vX.Y.Z release tags at {source}")
    return max(tags, key=lambda tag: tuple(int(n) for n in TAG.match(tag).groups()))


def fetch_release(source: str, tag: str, allow_unverified: bool) -> Path:
    """A verified checkout of one release tag, cached per version."""
    target = CACHE / tag
    if not (target / ".git").exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            shutil.rmtree(target)
        print(f"fetching {tag} from {source} …")
        clone = subprocess.run(["git", "clone", "-q", "--depth", "1", "--branch", tag, source, str(target)],
                               capture_output=True, text=True)
        if clone.returncode != 0:
            raise SystemExit(f"cannot fetch {tag}: {clone.stderr.strip()[:300]}")
    verify = subprocess.run(["git", "-C", str(target), "verify-tag", tag], capture_output=True, text=True)
    if verify.returncode == 0:
        print(f"verified: {tag} is signed by an allowed signer")
    elif allow_unverified:
        print(f"warning: the signature on {tag} could not be verified ({verify.stderr.strip()[:160]}); "
              "continuing because --allow-unverified was given")
    else:
        raise SystemExit(f"refusing {tag}: its signature could not be verified "
                         f"({verify.stderr.strip()[:200] or 'no signature'}). Configure gpg.ssh.allowedSignersFile "
                         "with the release signer, or pass --allow-unverified to accept it knowingly.")
    manifest_path = target / RELEASE_MANIFEST
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        files = release_machinery(target)
        bad = [rel for rel, digest in manifest["files"].items()
               if hashlib.sha256(files.get(rel, b"")).hexdigest() != digest]
        if bad:
            raise SystemExit(f"refusing {tag}: {len(bad)} file(s) differ from its RELEASE.json, e.g. {bad[0]}")
    return target


# ------------------------------------------------------------------ commands

def cmd_validate(args) -> int:
    return script("validate-bootstrap.py", "--check-context", cwd=project_root(args))


def cmd_context(args) -> int:
    flags = [flag for flag, on in (("--staged", args.staged), ("--check", args.check),
                                   ("--install-hook", args.install_hook)) if on]
    return script("gen-context.py", *flags, cwd=project_root(args))


def cmd_reconcile(args) -> int:
    project = project_root(args)
    return script("reconcile-project.py", "--project", str(project), "--pretty", cwd=project)


def cmd_version(args) -> int:
    project = project_root(args)
    marker = installation(project)
    print(f"installed {marker.get('bootstrapVersion') or Layout(project).path('version').read_text().strip()}"
          f" from {marker.get('source') or DEFAULT_SOURCE}")
    return 0


def cmd_upgrade(args) -> int:
    project = project_root(args)
    dirty = [line[3:] for line in git(project, "status", "--porcelain").stdout.splitlines()
             if line[3:].startswith((".agents/acp/", ".agents/bootstrap/")) or contained_path(line[3:])]
    if dirty and not args.force:
        print("refusing: these bootstrap files have uncommitted changes, which the upgrade would move or replace "
              "and so carry into its commit:\n  " + "\n  ".join(dirty[:20])
              + "\ncommit or discard them first, upgrade in a clean worktree "
              "(git worktree add ../upgrade origin/main), or pass --force", file=sys.stderr)
        return 2
    base = args.base
    if args.release:
        release = Path(args.release).expanduser().resolve()
    else:
        marker = installation(project)
        source = marker.get("source") or DEFAULT_SOURCE
        tag = args.to if args.to else latest_tag(source)
        if not tag.startswith("v"):
            tag = f"v{tag}"
        release = fetch_release(source, tag, args.allow_unverified)
        installed = marker.get("bootstrapVersion")
        if not base and installed and f"v{installed}" != tag:
            try:
                base = str(fetch_release(source, f"v{installed}", allow_unverified=True))
            except SystemExit as exc:
                print(f"note: no base for three-way comparison ({exc}); differing project files are kept")
    before = set(git(project, "status", "--porcelain", "-uall").stdout.splitlines())
    argv = ["--project", str(project), "--from", str(release)]
    if base:
        argv += ["--base", str(base)]
    argv.append("--apply" if (args.apply or args.commit) else "--check")
    code = script("upgrade-project.py", *argv, cwd=project)
    if code or not args.commit:
        return code
    return commit_upgrade(project, before)


def commit_upgrade(project: Path, before: set[str]) -> int:
    """Stage exactly what the upgrade changed, with a lock compiled from that index, and commit it."""
    after = git(project, "status", "--porcelain", "-uall").stdout.splitlines()
    changed = sorted({line[3:].split(" -> ")[-1] for line in after if line not in before}
                     | {line[3:] for line in after if line not in before and line.startswith(" D")})
    plan_dir = Layout(project).path("upgrades")
    plans = sorted(plan_dir.glob("*/upgrade-plan.json"), key=lambda p: p.stat().st_mtime)
    plan = json.loads(plans[-1].read_text(encoding="utf-8")) if plans else {}
    moved = [item["from"] for item in plan.get("layoutMigration", [])]
    paths = sorted(set(changed) | set(moved))
    if not paths:
        print("nothing to commit")
        return 0
    add = subprocess.run(["git", "-C", str(project), "add", "-A", "--", *paths], capture_output=True, text=True)
    if add.returncode != 0:
        print(f"could not stage the upgrade: {add.stderr.strip()[:300]}", file=sys.stderr)
        return 2
    lock = Layout(project).bootstrap_script("gen-context.py")
    subprocess.check_call([sys.executable, str(lock), "--staged"], cwd=project, stdout=subprocess.DEVNULL)
    for rel in (".agents/context-lock.json", ".agents/llms.txt", ".agents/llms-full.txt"):
        if rel == ".agents/context-lock.json" or git(project, "ls-files", "--error-unmatch", rel).returncode == 0:
            git(project, "add", "--", rel)
    version = plan.get("toVersion") or "the new release"
    lines = [f"chore(bootstrap): upgrade to formal-project-bootstrap {version}", ""]
    if moved:
        lines.append(f"Moves {len(moved)} bootstrap file(s) under .agents/; only AGENTS.md stays at the root.")
    references = sorted({item["path"] for item in plan.get("externalReferences", [])})
    if references:
        lines.append("Still quoting old bootstrap paths (not edited): " + ", ".join(references) + ".")
    lines.append("Applied with .agents/bootstrap/bootstrap upgrade --commit.")
    commit = subprocess.run(["git", "-C", str(project), "commit", "-q", "-m", "\n".join(lines)],
                            capture_output=True, text=True)
    if commit.returncode != 0:
        print(f"commit failed: {(commit.stderr or commit.stdout).strip()[:300]}", file=sys.stderr)
        return 2
    print(git(project, "log", "-1", "--format=committed %h %s").stdout.strip())
    print("publish it with: .agents/acp/acp push")
    return 0


def cmd_repair(args) -> int:
    project = project_root(args)
    marker = installation(project)
    installed = marker.get("bootstrapVersion")
    if args.release:
        release = Path(args.release).expanduser().resolve()
    else:
        if not installed:
            print("no installed version recorded; pass --from a release checkout", file=sys.stderr)
            return 2
        release = fetch_release(marker.get("source") or DEFAULT_SOURCE, f"v{installed}", args.allow_unverified)
    wanted = release_machinery(release)
    profile = {}
    try:
        profile = json.loads(Layout(project).path("profile").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        pass
    overrides = set(profile.get("bootstrapOverrides") or [])
    drifted = [rel for rel, content in sorted(wanted.items())
               if rel not in overrides and (project / rel).is_file() and (project / rel).read_bytes() != content]
    missing = [rel for rel in sorted(wanted) if rel not in overrides and not (project / rel).exists()]
    if not drifted and not missing:
        print("machinery matches the installed release; nothing to repair")
        return 0
    for rel in drifted:
        print(f"edited:  {rel}")
    for rel in missing:
        print(f"missing: {rel}")
    if not args.apply:
        print("dry run: pass --apply to restore these from the release (edited copies are backed up)")
        return 0
    backup = Layout(project).path("upgrades") / f"repair-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    for rel in drifted:
        destination = backup / rel
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(project / rel, destination)
    for rel in drifted + missing:
        path = project / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(wanted[rel])
    for executable in (".agents/acp/acp", ".agents/bootstrap/bootstrap"):
        if (project / executable).exists():
            (project / executable).chmod(0o755)
    print(f"restored {len(drifted) + len(missing)} file(s); edited copies are in {backup.relative_to(project)}")
    return 0


def cmd_uninstall(args) -> int:
    project = project_root(args)
    manifest_path = project / RELEASE_MANIFEST
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {"files": {}}
    removable, kept = [], []
    for rel, digest in sorted(manifest["files"].items()):
        path = project / rel
        if not path.is_file():
            continue
        same = hashlib.sha256(path.read_bytes()).hexdigest() == digest
        if rel.startswith(SHIPPED_SKILLS + "/") and not same:
            kept.append(rel)
        else:
            removable.append(rel)
    extra = [rel for rel in (RELEASE_MANIFEST, ".agents/context-lock.json", ".agents/llms.txt", ".agents/llms-full.txt")
             if (project / rel).exists()]
    skill_dirs = {"/".join(rel.split("/")[:3]) for rel in kept}
    print("bootstrap uninstall " + ("steps:" if args.apply else "plan:"))
    print(f"  - remove {len(removable) + len(extra)} bootstrap file(s): .agents/acp/, .agents/bootstrap/, "
          "unmodified shipped skills, the compiled context")
    if kept:
        print(f"  - keep modified skills: {', '.join(sorted(skill_dirs))}")
    print("  - keep .agents/project/ and .agents/records/ (yours)"
          + (f", and copy them to {args.export}" if args.export else ""))
    hooks = git(project, "rev-parse", "--git-path", "hooks").stdout.strip()
    hook = (project / hooks if hooks and not Path(hooks).is_absolute() else Path(hooks or ".git/hooks")) / "pre-commit"
    ours = hook.is_file() and "formal-project-bootstrap: compile context" in hook.read_text(errors="replace")
    if ours:
        print("  - remove the context pre-commit hook it installed")
    if not args.apply:
        print("dry run: pass --apply to uninstall (everything removed is backed up first)")
        return 0
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    backup = project / ".agents/.upgrades" / f"uninstall-{stamp}"
    if args.export:
        export = Path(args.export).expanduser().resolve()
        for name in (".agents/project", ".agents/records"):
            if (project / name).exists():
                shutil.copytree(project / name, export / name, dirs_exist_ok=True)
    for rel in removable + extra:
        destination = backup / rel
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(project / rel), str(destination))
    if ours:
        destination = backup / "pre-commit"
        shutil.move(str(hook), str(destination))
    for top in (".agents/acp", ".agents/bootstrap", SHIPPED_SKILLS):
        directory = project / top
        if directory.is_dir():
            # Interpreter caches the bootstrap's own scripts left behind are not anyone's files.
            for cache in sorted(directory.rglob("__pycache__"), key=lambda p: -len(p.parts)):
                shutil.rmtree(cache, ignore_errors=True)
            for sub in sorted((p for p in directory.rglob("*") if p.is_dir()), key=lambda p: -len(p.parts)):
                try:
                    sub.rmdir()
                except OSError:
                    pass
            try:
                directory.rmdir()
            except OSError:
                pass
    print(f"uninstalled; everything removed is in {backup.relative_to(project)}. "
          "If ACP still governs this repository, its owner unregisters the policy on the gateway.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(prog="bootstrap", description="Formal Project Bootstrap for this project.")
    parser.add_argument("--project", help="project root (default: the nearest one)")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("validate", help="check structure and the context lock").set_defaults(fn=cmd_validate)
    p = sub.add_parser("context", help="compile the context (llms.txt, llms-full.txt, context-lock.json)")
    p.add_argument("--staged", action="store_true", help="compile the commit being made (the Git index)")
    p.add_argument("--check", action="store_true")
    p.add_argument("--install-hook", action="store_true", help="install the pre-commit hook that runs --staged")
    p.set_defaults(fn=cmd_context)
    sub.add_parser("reconcile", help="read-only adoption report").set_defaults(fn=cmd_reconcile)
    sub.add_parser("version", help="the installed release and its source").set_defaults(fn=cmd_version)

    p = sub.add_parser("upgrade", help="fetch, verify and apply a release (a dry run unless --apply or --commit)")
    p.add_argument("--to", help="release tag, e.g. v0.7.0 (default: the latest)")
    p.add_argument("--from", dest="release", help="a local release checkout instead of fetching")
    p.add_argument("--base", help="the previous release checkout, for three-way comparison")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--commit", action="store_true", help="apply, then commit exactly the upgrade's files")
    p.add_argument("--allow-unverified", action="store_true", help="accept a release whose tag signature cannot be verified")
    p.add_argument("--force", action="store_true", help="proceed although bootstrap machinery has uncommitted edits")
    p.set_defaults(fn=cmd_upgrade)

    p = sub.add_parser("repair", help="restore edited or missing machinery to the installed release")
    p.add_argument("--from", dest="release", help="a local checkout of the installed release")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--allow-unverified", action="store_true")
    p.set_defaults(fn=cmd_repair)

    p = sub.add_parser("uninstall", help="remove the bootstrap, keeping the project's own files (dry run unless --apply)")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--export", help="also copy .agents/project and .agents/records here")
    p.set_defaults(fn=cmd_uninstall)

    args = parser.parse_args()
    return args.fn(args)
