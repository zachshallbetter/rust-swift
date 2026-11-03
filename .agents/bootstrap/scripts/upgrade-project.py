#!/usr/bin/env python3
"""Plan, apply, verify, and roll back a non-destructive bootstrap upgrade.

A legacy-layout project (bootstrap 0.6.x or older) is moved into the contained
layout on the way: bootstrap's files go under .agents/, known paths quoted in the
project's own AGENTS.md and .agents/project/ files are rewritten, and every other
project file that still quotes an old path is reported, never edited.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layout import (  # noqa: E402
    LEGACY_DIRECTORIES, PATHS, PROJECT_OWNED_DIRS, RECORD_FILES, TEMPLATES, Layout, contained_path,
    find_project_root, release_machinery, rewrite_paths, rewrite_split_paths, text_rewrites,
)

ROOT = find_project_root(Path(__file__).resolve().parent)
CONTAINED = Layout(Path("."), contained=True)
# Project-owned outside the project directories: the root contract.
PROJECT_OWNED_FILES = frozenset({"AGENTS.md"})
# Where the migration rewrites quoted legacy paths. Records are history and are never rewritten.
REWRITE_SCOPE = ("AGENTS.md", ".agents/project/")


def rewritable(rel: str) -> bool:
    return rel == "AGENTS.md" or rel.startswith(REWRITE_SCOPE[1])


class UpgradeError(RuntimeError):
    pass


# ---------------------------------------------------------------- releases

def is_contained_release(release: Path) -> bool:
    return (release / PATHS["version"][0]).is_file()


def version(release: Path) -> str:
    path = release / (PATHS["version"][0] if is_contained_release(release) else "VERSION")
    try:
        value = path.read_text(encoding="utf-8").strip()
    except OSError as exc:
        raise UpgradeError(f"cannot read release VERSION: {exc}") from exc
    if not value:
        raise UpgradeError(f"release VERSION is empty: {release}")
    return value


def validate_release(release: Path) -> None:
    required = [PATHS["version"][0], f"{TEMPLATES}/profile.json", ".agents/bootstrap/scripts/init-project.py",
                ".agents/bootstrap/scripts/gen-context.py", ".agents/bootstrap/scripts/validate-bootstrap.py",
                ".agents/acp/acp"]
    missing = [rel for rel in required if not (release / rel).is_file()]
    if missing:
        raise UpgradeError(f"release is incomplete; missing: {', '.join(missing)}")


def load_legacy_initializer(release: Path):
    path = release / "scripts/init-project.py"
    spec = importlib.util.spec_from_file_location("bootstrap_legacy_initializer", path)
    if spec is None or spec.loader is None:
        raise UpgradeError(f"initializer missing from release: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# ---------------------------------------------------------------- projects

def read(path: Path) -> bytes | None:
    return path.read_bytes() if path.is_file() else None


def render(text: str, project_id: str, project_name: str) -> bytes:
    return text.replace("<PROJECT_ID>", project_id).replace("<PROJECT_NAME>", project_name).encode()


def project_identity(project: Path) -> tuple[str, str, str]:
    layout = Layout(project)
    try:
        marker = json.loads(layout.path("installation").read_text(encoding="utf-8"))
        profile = json.loads(layout.path("profile").read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise UpgradeError(f"project bootstrap metadata is unreadable: {exc}") from exc
    project_id = str(marker.get("projectId") or profile.get("projectId") or "")
    project_name = str(marker.get("projectName") or profile.get("projectName") or "")
    installed = str(marker.get("bootstrapVersion") or profile.get("profileVersion") or "")
    if not project_id or not project_name or not installed:
        raise UpgradeError("project metadata must contain projectId, projectName, and bootstrapVersion")
    return project_id, project_name, installed


def merge_defaults(default: object, current: object) -> object:
    if isinstance(default, dict) and isinstance(current, dict):
        merged = {key: merge_defaults(value, current[key]) if key in current else value
                  for key, value in default.items()}
        merged.update({key: value for key, value in current.items() if key not in merged})
        return merged
    return current


def merged_profile(template: dict, current: dict, project_id: str, project_name: str) -> dict:
    """The release's new defaults added, every project value kept, no version stamped (bootstrap#22)."""
    current = dict(current)
    if "profileVersion" in current:                  # legacy: the version now lives in installation.json
        current.pop("profileVersion")
    current_text, _ = rewrite_paths(json.dumps(current))
    current = json.loads(current_text)
    provider = current.get("authorizationProvider")
    if isinstance(provider, dict):
        provider.pop("checkScript", None)
        if provider.get("clientScript") in (".agents/acp/lib/acp.py", "scripts/acp.py"):
            provider["clientScript"] = ".agents/acp/acp"
    profile = merge_defaults(template, current)
    profile["projectId"] = project_id
    profile["projectName"] = project_name
    provider = profile.get("authorizationProvider")
    if isinstance(provider, dict) and provider.get("gatewayEndpointSet"):
        provider.pop("gatewayUrlEnv", None)
        provider.pop("gatewayTokenEnv", None)
    return profile


def target_map(release: Path, project: Path, project_id: str, project_name: str) -> dict[str, bytes]:
    """Every file the release wants in a contained project, at its contained path."""
    layout = Layout(project, contained=True)
    result = release_machinery(release)
    templates = release / TEMPLATES
    result["AGENTS.md"] = render((release / "AGENTS.md").read_text(encoding="utf-8"), project_id, project_name)
    result[layout.rel("intent")] = render((templates / "intent.md").read_text(encoding="utf-8"),
                                          project_id, project_name)
    for name, target in (("contracts", layout.rel("contracts")), ("rules", layout.rel("rules"))):
        for path in sorted((templates / name).rglob("*")):
            if path.is_file():
                result[f"{target}/{path.relative_to(templates / name).as_posix()}"] = path.read_bytes()
    for name in RECORD_FILES:                         # records belong to the project: create, never rewrite
        rel = f"{layout.rel('records')}/{name}"
        result[rel] = read(project / rel) or read(project / "records" / name) or b""
    template = json.loads((templates / "profile.json").read_text(encoding="utf-8"))
    current_path = Layout(project).path("profile")
    current = json.loads(current_path.read_text(encoding="utf-8")) if current_path.is_file() else {}
    profile = merged_profile(template, current, project_id, project_name)
    result[layout.rel("profile")] = (json.dumps(profile, indent=2) + "\n").encode()
    context = json.loads((templates / "context-sources.json").read_text(encoding="utf-8"))
    result[layout.rel("context_sources")] = (json.dumps(context, indent=2) + "\n").encode()
    result[layout.rel("formal_resources")] = render(
        (templates / "formal-resources.json").read_text(encoding="utf-8"), project_id, project_name)
    result[layout.rel("bindings")] = render((templates / "bindings.yaml").read_text(encoding="utf-8"),
                                            project_id, project_name)
    return result


def legacy_target_map(release: Path, project: Path, project_id: str, project_name: str) -> dict[str, bytes]:
    """What a 0.6.x-or-older release would install, keyed by the contained paths those files move to."""
    initializer = load_legacy_initializer(release)
    legacy: dict[str, bytes] = {}
    for rel in initializer.COPY_FILES:
        legacy[rel] = (release / rel).read_bytes()
    for rel in initializer.COPY_DIRS:
        source = release / rel
        for path in sorted(source.rglob("*")):
            if path.is_file():
                legacy[str(Path(rel) / path.relative_to(source))] = path.read_bytes()
    for name in initializer.COPY_DOCS:
        legacy[f"docs/{name}"] = (release / "docs" / name).read_bytes()
    for name in initializer.SCRIPT_FILES:
        legacy[f"scripts/{name}"] = (release / "scripts" / name).read_bytes()
    legacy["VERSION"] = (release / "VERSION").read_bytes()
    legacy["PROJECT_INTENT.md"] = render((release / "PROJECT_INTENT_TEMPLATE.md").read_text(encoding="utf-8"),
                                         project_id, project_name)
    legacy["FORMAL_RESOURCE_MANIFEST.json"] = render(
        (release / "FORMAL_RESOURCE_MANIFEST_TEMPLATE.json").read_text(encoding="utf-8"), project_id, project_name)
    legacy["bindings/PROJECT_BINDINGS.yaml"] = render(
        (release / "bindings/PROJECT_BINDINGS_TEMPLATE.yaml").read_text(encoding="utf-8"), project_id, project_name)
    context = json.loads((release / "CONTEXT_SOURCES_TEMPLATE.json").read_text(encoding="utf-8"))
    legacy["CONTEXT_SOURCES.json"] = (json.dumps(context, indent=2) + "\n").encode()
    result = {}
    for rel, content in legacy.items():
        moved = contained_path(rel) or rel
        # Text the migration rewrites compares in its rewritten form.
        if rewritable(moved):
            text, count = rewrite_paths(content.decode("utf-8", "surrogateescape"))
            content = text.encode("utf-8", "surrogateescape") if count else content
        result[moved] = content
    return result


def base_map(base: Path, project: Path, project_id: str, project_name: str) -> dict[str, bytes]:
    try:
        if is_contained_release(base):
            return target_map(base, project, project_id, project_name)
        return legacy_target_map(base, project, project_id, project_name)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, AttributeError, UpgradeError) as exc:
        raise UpgradeError(f"cannot render base release {base}: {exc}") from exc


def project_owned(rel: str, overrides: frozenset[str] = frozenset()) -> bool:
    return rel in PROJECT_OWNED_FILES or rel in overrides or rel.startswith(tuple(d + "/" for d in PROJECT_OWNED_DIRS))


def project_overrides(project: Path) -> frozenset[str]:
    """Machinery a project has taken over: the profile's `bootstrapOverrides`, in contained paths."""
    try:
        profile = json.loads(Layout(project).path("profile").read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return frozenset()
    value = profile.get("bootstrapOverrides") or []
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise UpgradeError("profile bootstrapOverrides must be a list of relative paths")
    return frozenset(contained_path(item) or item for item in value)


# ---------------------------------------------------------------- the migration

def migration_moves(project: Path) -> list[tuple[str, str]]:
    """(legacy, contained) for every legacy bootstrap file present in a legacy project."""
    if Layout(project).contained:
        return []
    listed = subprocess.run(["git", "-C", str(project), "ls-files", "-z", "--cached", "--others",
                             "--exclude-standard"], capture_output=True)
    names = ({name for name in listed.stdout.decode("utf-8", "surrogateescape").split("\0") if name}
             if listed.returncode == 0 else
             {path.relative_to(project).as_posix() for path in project.rglob("*") if path.is_file()})
    names.add(PATHS["installation"][1])               # untracked in some projects, always moved
    moves = []
    for rel in sorted(names):
        target = contained_path(rel)
        if target and (project / rel).is_file() and not (project / rel).is_symlink():
            moves.append((rel, target))
    return moves


def external_references(project: Path, owned: set[str]) -> list[dict]:
    """Project files that still quote a legacy bootstrap path, with the replacement. Never edited."""
    listed = subprocess.run(["git", "-C", str(project), "ls-files", "-z"], capture_output=True)
    names = [n for n in listed.stdout.decode("utf-8", "surrogateescape").split("\0") if n] if listed.returncode == 0 else []
    hooks = subprocess.run(["git", "-C", str(project), "config", "core.hooksPath"], capture_output=True, text=True)
    findings = []
    olds = [(old, new) for old, new in text_rewrites()]
    for rel in names:
        if rel in owned or rel.startswith((".agents/",)) or contained_path(rel):
            continue
        path = project / rel
        # Any text file: Rust, Go, Makefiles and workflow files quote paths as readily as Python does.
        if not path.is_file() or path.is_symlink() or path.stat().st_size > 1_000_000:
            continue
        try:
            data = path.read_bytes()
            if b"\0" in data[:8192]:
                continue
            lines = data.decode("utf-8").splitlines()
        except (OSError, UnicodeDecodeError):
            continue
        for number, line in enumerate(lines, 1):
            new_line, count = rewrite_paths(line)
            new_line, split = rewrite_split_paths(new_line)
            if count or split:
                hits = [old for old, _ in olds if old in line or all(part in line for part in old.split("/"))]
                findings.append({"path": rel, "line": number, "quotes": hits[:3], "replacement": new_line.strip()[:240]})
    if hooks.returncode == 0 and hooks.stdout.strip():
        findings.append({"path": f"core.hooksPath={hooks.stdout.strip()}", "line": 0, "quotes": [],
                         "replacement": "check the hooks there for scripts/... paths"})
    return findings


# ---------------------------------------------------------------- classification

def classify(project: Path, target: dict[str, bytes], base: dict[str, bytes] | None,
             moved_from: dict[str, str]) -> dict:
    overrides = project_overrides(project)
    safe, conflicts, unchanged, additions, preserved, replaced = [], [], [], [], [], []

    def current_of(rel: str) -> bytes | None:
        source = moved_from.get(rel, rel)
        content = read(project / source)
        if content is not None and source != rel and rewritable(rel):
            text, count = rewrite_paths(content.decode("utf-8", "surrogateescape"))
            content = text.encode("utf-8", "surrogateescape") if count else content
        return content

    for rel, desired in sorted(target.items()):
        current = current_of(rel)
        if current == desired:
            unchanged.append(rel)
            continue
        if current is None:
            additions.append(rel)
            safe.append(rel)
            continue
        if rel == CONTAINED.rel("profile"):
            safe.append(rel)
            continue
        original = base.get(rel) if base is not None else None
        if original is not None and original == current:
            safe.append(rel)
            continue
        if original is not None and original == desired:
            preserved.append(rel)
            continue
        if project_owned(rel, overrides):
            conflicts.append(rel)
            continue
        safe.append(rel)
        replaced.append(rel)

    removed = []
    if base is not None:
        for rel, content in sorted(base.items()):
            if rel not in target and current_of(rel) == content:
                removed.append(rel)
    return {"safe": safe, "additions": additions, "unchanged": unchanged, "preserved": preserved,
            "replacedLocalEdits": replaced, "conflicts": conflicts, "removedCandidates": removed}


@contextlib.contextmanager
def resolved_base(release: Path, base: Path | None, installed: str):
    """The explicit --base, else the release checkout's tag for the installed version."""
    if base is not None:
        yield base, str(base)
        return
    tag = f"v{installed}"
    probe = subprocess.run(["git", "-C", str(release), "rev-parse", "--verify", "--quiet",
                            f"refs/tags/{tag}^{{commit}}"], capture_output=True, text=True)
    if probe.returncode != 0:
        yield None, None
        return
    with tempfile.TemporaryDirectory(prefix="bootstrap-base-") as tmp:
        archive = subprocess.run(["git", "-C", str(release), "archive", "--format=tar", tag],
                                 capture_output=True, check=True)
        subprocess.run(["tar", "-x", "-C", tmp], input=archive.stdout, check=True)
        yield Path(tmp), f"{release} at tag {tag}"


def plan(project: Path, release: Path, base: Path | None) -> dict:
    validate_release(release)
    project_id, project_name, installed = project_identity(project)
    target_version = version(release)
    moves = migration_moves(project)
    if installed == target_version and not moves:
        return {"current": True, "fromVersion": installed, "toVersion": target_version}
    target = target_map(release, project, project_id, project_name)
    moved_from = {new: old for old, new in moves}
    collisions = [new for old, new in moves if (project / new).exists()]
    if collisions:
        raise UpgradeError("contained paths already exist, so the layout cannot be moved safely: "
                           + ", ".join(collisions[:10]))
    with resolved_base(release, base, installed) as (base_dir, base_label):
        original = base_map(base_dir, project, project_id, project_name) if base_dir else None
    result = classify(project, target, original, moved_from)
    owned = {rel for rel in target if project_owned(rel)} | {old for old, _ in moves}
    return {
        "format": "2",
        "bootstrap": "formal-project-bootstrap",
        "fromVersion": installed,
        "toVersion": target_version,
        "project": str(project),
        "release": str(release),
        "base": base_label,
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "layoutMigration": [{"from": old, "to": new} for old, new in moves],
        "externalReferences": external_references(project, owned) if moves else [],
        "classification": result,
        "targetFiles": sorted(target),
    }


# ---------------------------------------------------------------- applying

class Transaction:
    """Every write, move and removal, undoable: originals are backed up, created paths recorded."""

    def __init__(self, project: Path, root: Path):
        self.project, self.root = project, root
        self.backup = root / "backup"
        if self.backup.exists():
            shutil.rmtree(self.backup)
        self.backup.mkdir(parents=True)
        self.created: list[str] = []
        self.saved: set[str] = set()

    def _save(self, rel: str) -> None:
        source = self.project / rel
        if rel in self.saved or rel in self.created:
            return
        if source.is_file():
            destination = self.backup / rel
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
            self.saved.add(rel)
        else:
            self.created.append(rel)

    def write(self, rel: str, content: bytes) -> None:
        self._save(rel)
        destination = self.project / rel
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as tmp:
            tmp.write(content)
        os.replace(tmp.name, destination)

    def move(self, old: str, new: str) -> None:
        self._save(old)
        self._save(new)
        destination = self.project / new
        destination.parent.mkdir(parents=True, exist_ok=True)
        os.replace(self.project / old, destination)

    def finish(self) -> None:
        (self.root / "transaction.json").write_text(
            json.dumps({"created": self.created, "saved": sorted(self.saved)}, indent=2) + "\n", encoding="utf-8")


def tool(project: Path, name: str) -> Path:
    return Layout(project).bootstrap_script(name)


def apply_plan(project: Path, release: Path, plan_data: dict) -> None:
    project_id, project_name, _ = project_identity(project)
    target = target_map(release, project, project_id, project_name)
    release_version = plan_data["toVersion"]
    upgrade_root = project / CONTAINED.rel("upgrades") / release_version
    for rel in plan_data["classification"]["conflicts"]:
        candidate = upgrade_root / "candidates" / rel
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_bytes(target[rel])
    tx = Transaction(project, upgrade_root)
    try:
        for move in plan_data["layoutMigration"]:
            tx.move(move["from"], move["to"])
        for move in plan_data["layoutMigration"]:
            rel = move["to"]
            if rewritable(rel):
                content = (project / rel).read_bytes()
                text, count = rewrite_paths(content.decode("utf-8", "surrogateescape"))
                if count:
                    tx.write(rel, text.encode("utf-8", "surrogateescape"))
        if plan_data["layoutMigration"] and (project / "AGENTS.md").is_file():
            text, count = rewrite_paths((project / "AGENTS.md").read_text(encoding="utf-8"))
            if count:
                tx.write("AGENTS.md", text.encode())
        for rel in plan_data["classification"]["safe"]:
            tx.write(rel, target[rel])
        for executable in (".agents/acp/acp", ".agents/bootstrap/bootstrap"):
            if (project / executable).is_file():
                (project / executable).chmod(0o755)
        marker_path = project / CONTAINED.rel("installation")
        marker = json.loads(marker_path.read_text(encoding="utf-8"))
        marker["format"] = "3"
        marker["bootstrapVersion"] = release_version
        marker["lastUpgrade"] = {"fromVersion": plan_data["fromVersion"], "toVersion": release_version}
        marker.setdefault("source", "https://github.com/zachshallbetter/formal-project-bootstrap")
        tx.write(CONTAINED.rel("installation"), (json.dumps(marker, indent=2) + "\n").encode())
        remove_empty_legacy_directories(project)
        ensure_gitignore(project, tx)
        tx.finish()
        (upgrade_root / "upgrade-plan.json").write_text(json.dumps(plan_data, indent=2) + "\n", encoding="utf-8")
        subprocess.check_call([sys.executable, str(tool(project, "gen-context.py"))], cwd=project)
        subprocess.check_call([sys.executable, str(tool(project, "validate-bootstrap.py")), "--check-context"],
                              cwd=project)
    except Exception:
        tx.finish()
        rollback(project, release_version)
        raise


def remove_empty_legacy_directories(project: Path) -> None:
    for name in LEGACY_DIRECTORIES:
        directory = project / name
        if not directory.is_dir():
            continue
        for sub in sorted((p for p in directory.rglob("*") if p.is_dir()), key=lambda p: -len(p.parts)):
            with contextlib.suppress(OSError):
                sub.rmdir()
        with contextlib.suppress(OSError):
            directory.rmdir()


def ensure_gitignore(project: Path, tx: Transaction) -> None:
    lines = [".agents/records/local/", ".agents/.upgrades/"]
    path = project / ".gitignore"
    existing = path.read_text(encoding="utf-8").splitlines() if path.exists() else []
    missing = [line for line in lines if line not in existing]
    if missing:
        block = ["", "# formal-project-bootstrap: per-machine ACP state and upgrade backups", *missing]
        tx.write(".gitignore", ("\n".join(existing + block) + "\n").encode())


def rollback(project: Path, release_version: str) -> None:
    for upgrades in (CONTAINED.rel("upgrades"), PATHS["upgrades"][1]):
        root = project / upgrades / release_version
        if (root / "backup").exists():
            break
    else:
        raise UpgradeError(f"upgrade backup not found for {release_version}")
    record = root / "transaction.json"
    created = json.loads(record.read_text(encoding="utf-8"))["created"] if record.exists() else []
    for rel in created:
        with contextlib.suppress(FileNotFoundError):
            (project / rel).unlink()
    backup = root / "backup"
    for path in sorted(backup.rglob("*")):
        if path.is_file():
            destination = project / path.relative_to(backup)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
    for rel in created:
        parent = (project / rel).parent
        while parent != project:
            try:
                parent.rmdir()
            except OSError:
                break
            parent = parent.parent
    generator = tool(project, "gen-context.py")
    validator = tool(project, "validate-bootstrap.py")
    if generator.exists() and validator.exists():
        subprocess.check_call([sys.executable, str(generator)], cwd=project)
        subprocess.check_call([sys.executable, str(validator), "--check-context"], cwd=project)


# ---------------------------------------------------------------- CLI

def summarize(plan_data: dict, applied: bool) -> None:
    """One line per kind of outcome, on stderr so stdout stays the plan document."""
    classes = plan_data["classification"]
    where = f"{CONTAINED.rel('upgrades')}/{plan_data['toVersion']}"
    verb = "updated" if applied else "would update"
    lines = [f"{plan_data['fromVersion']} -> {plan_data['toVersion']}: {verb} {len(classes['safe'])} file(s); "
             f"base {plan_data['base'] or 'unavailable (pass --base for three-way comparison)'}"]
    if plan_data["layoutMigration"]:
        lines.append(f"layout: {'moved' if applied else 'will move'} {len(plan_data['layoutMigration'])} "
                     "bootstrap file(s) under .agents/; only AGENTS.md stays at the root")
    if classes["replacedLocalEdits"]:
        lines.append(f"bootstrap machinery with local edits {'replaced' if applied else 'to replace'}; "
                     f"the previous copies are in {where}/backup: " + ", ".join(classes["replacedLocalEdits"]))
    if classes["conflicts"]:
        lines.append(f"project-owned files kept as they are; the release copies to review are in "
                     f"{where}/candidates: " + ", ".join(classes["conflicts"]))
    if classes["removedCandidates"]:
        lines.append("no longer shipped, left in place: " + ", ".join(classes["removedCandidates"]))
    references = plan_data["externalReferences"]
    if references:
        files = sorted({item["path"] for item in references})
        lines.append(f"{len(references)} line(s) in {len(files)} project file(s) still quote old bootstrap paths; "
                     "update them (not edited by the upgrade): " + ", ".join(files))
        for item in references[:40]:
            lines.append(f"  {item['path']}:{item['line']}: {item['replacement']}")
    for line in lines:
        print(line, file=sys.stderr)


def run_locked(args, project: Path, release: Path, base: Path | None) -> int:
    try:
        if args.verify:
            subprocess.check_call([sys.executable, str(tool(project, "validate-bootstrap.py")), "--check-context"],
                                  cwd=project)
            return 0
        if args.rollback:
            marker = json.loads(Layout(project).path("installation").read_text(encoding="utf-8"))
            target_version = str((marker.get("lastUpgrade") or {}).get("toVersion") or "")
            if not target_version:
                raise UpgradeError("project has no recorded upgrade to roll back")
            rollback(project, target_version)
            return 0
        plan_data = plan(project, release, base)
        if plan_data.get("current"):
            print(json.dumps(plan_data, indent=2))
            print(f"current: the project already has bootstrap {plan_data['toVersion']}; nothing to do "
                  "(.agents/bootstrap/bootstrap repair restores edited machinery)", file=sys.stderr)
            return 0
        if args.apply:
            path = project / CONTAINED.rel("upgrades") / plan_data["toVersion"] / "plan.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(plan_data, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(plan_data, indent=2), flush=True)
        if args.apply:
            apply_plan(project, release, plan_data)
            print(f"applied bootstrap upgrade to {plan_data['toVersion']}")
            summarize(plan_data, applied=True)
            return 0
        summarize(plan_data, applied=False)
        if not args.check:
            print("plan only; pass --apply to mutate the project", file=sys.stderr)
        return 3 if plan_data["classification"]["conflicts"] else 0
    except (OSError, subprocess.CalledProcessError, UpgradeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", default=None, help="project root (default: the nearest one)")
    parser.add_argument("--from", dest="release", required=True, help="target bootstrap release directory")
    parser.add_argument("--base", help="previous bootstrap release directory for three-way comparison")
    parser.add_argument("--check", "--plan", action="store_true", dest="check")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--rollback", action="store_true")
    args = parser.parse_args()
    project = Path(args.project).resolve() if args.project else find_project_root()
    release = Path(args.release).resolve()
    base = Path(args.base).resolve() if args.base else None
    lock_dir = project / CONTAINED.rel("upgrades")
    try:
        lock_dir.mkdir(parents=True, exist_ok=True)
        ignore = lock_dir / ".gitignore"
        if not ignore.exists():
            ignore.write_text("# Local upgrade plans, backups and candidates.\n*\n", encoding="utf-8")
        with (lock_dir / ".lock").open("w", encoding="utf-8") as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
            return run_locked(args, project, release, base)
    except (OSError, subprocess.CalledProcessError, UpgradeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
