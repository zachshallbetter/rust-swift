"""Owner-approved, local-only ACP opt-outs for bounded experimentation."""

from __future__ import annotations

import json
import os
import subprocess
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT
LOCAL_ENVIRONMENTS = frozenset({"local", "preview"})
LOCAL_ACTIONS = frozenset({"repository.read", "source.inspect", "local.analysis", "local.test"})
MAX_TTL_SECONDS = 8 * 60 * 60
MAX_LEDGER_BYTES = 1024 * 1024
MAX_LEDGER_EVENTS = 1000


def git(*args: str) -> str | None:
    try:
        return subprocess.check_output(["git", "-C", str(PROJECT_ROOT), *args], text=True,
                                       stderr=subprocess.DEVNULL).strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


def configure_project_root(path: Path) -> None:
    global PROJECT_ROOT
    PROJECT_ROOT = path.resolve()


def current_branch() -> str | None:
    return git("symbolic-ref", "--quiet", "--short", "HEAD")


def ledger_path() -> Path | None:
    path = git("rev-parse", "--git-path", "acp-owner-overrides.jsonl")
    if not path:
        return None
    target = Path(path)
    return target if target.is_absolute() else PROJECT_ROOT / target


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _read_events(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    if path.is_symlink() or path.stat().st_size > MAX_LEDGER_BYTES:
        raise ValueError("local ACP override ledger is unsafe or exceeds its size limit")
    events = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        try:
            item = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"local ACP override ledger is invalid at line {number}") from exc
        if not isinstance(item, dict) or item.get("version") != 1:
            raise ValueError(f"local ACP override ledger has an unsupported record at line {number}")
        if number > MAX_LEDGER_EVENTS:
            raise ValueError("local ACP override ledger exceeds its event limit")
        events.append(item)
    return events


def _append(path: Path, event: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink():
        raise ValueError("local ACP override ledger cannot be a symlink")
    encoded = json.dumps(event, sort_keys=True, separators=(",", ":")) + "\n"
    if path.exists() and path.stat().st_size + len(encoded.encode("utf-8")) > MAX_LEDGER_BYTES:
        raise ValueError("local ACP override ledger reached its size limit")
    if path.exists() and len(_read_events(path)) >= MAX_LEDGER_EVENTS:
        raise ValueError("local ACP override ledger reached its event limit")
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND | nofollow, 0o600)
    with os.fdopen(fd, "a", encoding="utf-8") as stream:
        stream.write(encoded)
    try:
        path.chmod(0o600)
    except OSError:
        pass


def grant(*, project_id: str, owner: str, repository: str, branch: str | None,
          environment: str, reason: str, expires_in: int, authorized_by: str,
          now: datetime | None = None) -> dict[str, Any]:
    if environment not in LOCAL_ENVIRONMENTS:
        raise ValueError("offline overrides are limited to local and local-preview environments")
    if not (project_id and owner and repository and reason.strip() and authorized_by.strip()):
        raise ValueError("project, repository, reason, and owner identity are required")
    if len(project_id) > 100 or len(owner) > 100 or len(repository) > 100 \
            or len(reason.strip()) > 240 or len(authorized_by.strip()) > 120:
        raise ValueError("local owner override fields exceed their size limits")
    if expires_in < 60 or expires_in > MAX_TTL_SECONDS:
        raise ValueError(f"expires_in must be between 60 and {MAX_TTL_SECONDS} seconds")
    path = ledger_path()
    if path is None:
        raise ValueError("not inside a Git worktree; local override cannot be recorded")
    now = now or _now()
    identifier = "local_override_" + os.urandom(12).hex()
    record = {
        "version": 1,
        "event": "GRANTED",
        "override_id": identifier,
        "project_id": project_id,
        "repository": {"owner": owner, "name": repository},
        "branch": branch or "*",
        "environment": environment,
        "actions": ["*"],
        "reason": reason.strip()[:240],
        "authorized_by": authorized_by.strip()[:120],
        "created_at": now.isoformat().replace("+00:00", "Z"),
        "expires_at": (now + timedelta(seconds=expires_in)).isoformat().replace("+00:00", "Z"),
    }
    _append(path, record)
    return record


def revoke(override_id: str, *, authorized_by: str, reason: str = "owner revoked") -> dict[str, Any]:
    path = ledger_path()
    if path is None:
        raise ValueError("not inside a Git worktree; local override cannot be recorded")
    active = {item.get("override_id"): item for item in _read_events(path)
              if item.get("event") == "GRANTED"}
    if override_id not in active:
        raise ValueError("override id was not found in this worktree's ledger")
    if not authorized_by.strip() or len(authorized_by) > 120 or not reason.strip() or len(reason) > 240:
        raise ValueError("local override revocation fields exceed their size limits")
    event = {"version": 1, "event": "REVOKED", "override_id": override_id,
             "authorized_by": authorized_by.strip()[:120], "reason": reason.strip()[:240],
             "revoked_at": _now().isoformat().replace("+00:00", "Z")}
    _append(path, event)
    return event


def active(*, project_id: str, owner: str, repository: str, branch: str | None,
           environment: str, action: str | None = None,
           now: datetime | None = None) -> dict[str, Any] | None:
    if environment not in LOCAL_ENVIRONMENTS:
        return None
    path = ledger_path()
    if path is None or not path.exists():
        return None
    now = now or _now()
    grants: dict[str, dict[str, Any]] = {}
    revoked: set[str] = set()
    for item in _read_events(path):
        identifier = item.get("override_id")
        if item.get("event") == "GRANTED" and identifier:
            grants[identifier] = item
        elif item.get("event") == "REVOKED" and identifier:
            revoked.add(identifier)
    for identifier, item in reversed(list(grants.items())):
        scope = item.get("repository") or {}
        if identifier in revoked or item.get("project_id") != project_id:
            continue
        if (scope.get("owner", "").lower(), scope.get("name", "").lower()) != (
                owner.lower(), repository.lower()):
            continue
        if item.get("branch") != "*" and (not branch or item.get("branch") != branch):
            continue
        if item.get("environment") != environment:
            continue
        if action and "*" not in (item.get("actions") or []) and action not in item["actions"]:
            continue
        try:
            expires = datetime.fromisoformat(item["expires_at"].replace("Z", "+00:00"))
        except (KeyError, ValueError, TypeError):
            continue
        if now < expires:
            return item
    return None


def inferred_environment(action: str) -> str:
    if action == "production.deploy":
        return "production"
    if action.startswith(("git.", "database.")):
        return "remote"
    return "local" if action in LOCAL_ACTIONS else "remote"


def valid_action_environment(action: str, environment: str) -> bool:
    if action.startswith("git."):
        return environment in {"remote", "preview", "staging", "production"}
    if action.startswith("database."):
        return environment in {"preview", "staging", "production"}
    if action == "production.deploy":
        return environment in {"preview", "production"}
    if action in LOCAL_ACTIONS:
        return environment in LOCAL_ENVIRONMENTS
    return environment in {"remote", "preview", "staging", "production"}
