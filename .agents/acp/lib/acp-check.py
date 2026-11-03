#!/usr/bin/env python3
"""Verify a project's acp-gateway wiring without printing secrets.

Checks, in order and stopping at the first failure:

  gate        GET /health                         liveness; app_configured reported
  auth        GET /internal/app-status            the token is accepted
  owner       GET /internal/projects?owner=       the owner's boards are readable
  board       (from that list)                    the bound board is present and allowed
  notice      GET /notice?owner=&name=            a policy governs this repository (required while the
                                                  profile is failClosed; --allow-ungoverned to downgrade)

Exit codes: 0 ok, 2 configuration missing, 3 gateway refused (body printed), 4 unreachable,
5 not governed (wired, but no ACP policy names this repository).
With --record, appends an evidence record (gate B0-acp) to the project's evidence ledger.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlencode

import acp_override
import acp_paths
import acp as acp_client

ROOT = acp_paths.find_project_root()
SECRET_KEYS = ("ACP_GATEWAY_TOKEN",)


def load_env_file(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        if key.startswith("export "):
            key = key[7:].strip()
        value = value.strip()
        if value[:1] in ('"', "'"):
            value = value[1:].split(value[0], 1)[0]
        else:
            value = value.split(" #", 1)[0].split("\t#", 1)[0].strip()  # inline comment
        values[key] = value
    return values


def settings() -> dict:
    profile = {}
    profile_path = acp_paths.profile_path(ROOT)
    if profile_path.exists():
        profile = json.loads(profile_path.read_text(encoding="utf-8"))
    acp = profile.get("authorizationProvider", {}) or {}
    board_env_file = acp.get("boardEnvFile", ".agents/board.env")

    env = {}
    env.update(load_env_file(ROOT / board_env_file))
    env.update(load_env_file(ROOT / ".env.local"))
    env.update({k: v for k, v in os.environ.items() if k.startswith("ACP_") or k.startswith("GH_PROJECT_")})

    board = acp.get("board") or {}
    owner = env.get("ACP_BOARD_OWNER") or env.get("GH_PROJECT_OWNER") or board.get("owner")
    number = env.get("ACP_BOARD_NUMBER") or env.get("GH_PROJECT_NUMBER") or board.get("number")
    repository_remote = env.get("ACP_REMOTE_URL") or git_repo_remote()
    repo_owner, repo_name = git_repo_identity(repository_remote)
    owner_override = env.get("ACP_REPOSITORY_OWNER")
    name_override = env.get("ACP_REPOSITORY_NAME")
    identity_error = None
    if repository_remote:
        if owner_override and repo_owner and owner_override.lower() != repo_owner.lower():
            identity_error = "ACP_REPOSITORY_OWNER conflicts with ACP_REMOTE_URL or origin"
        elif name_override and repo_name and name_override.lower() != repo_name.lower():
            identity_error = "ACP_REPOSITORY_NAME conflicts with ACP_REMOTE_URL or origin"
    else:
        repo_owner = owner_override or owner
        repo_name = name_override
    shared = acp_client.settings()
    return {
        "url": shared["url"],
        "token": shared["token"],
        "endpoints": shared.get("endpoints", []),
        "endpoint_set": shared.get("endpoint_set"),
        "machine_registry_path": shared.get("machine_registry_path"),
        "endpoint_problems": shared.get("endpoint_problems", []),
        "secrets": shared.get("secrets", []),
        "owner": owner,
        "number": str(number) if number not in (None, "") else None,
        "repo_owner": repo_owner,
        "repo_name": repo_name,
        "identity_error": identity_error or shared.get("identity_error"),
        "fail_closed": acp.get("failClosed", True) is not False,
        "project_id": profile.get("projectId"),
        "branch": acp_override.current_branch(),
        "token_env": shared.get("token_env", "ACP_GATEWAY_TOKEN"),
    }


def git_repo_remote() -> str | None:
    """Return origin when no committed canonical remote is configured."""
    try:
        return subprocess.check_output(
            ["git", "-C", str(ROOT), "remote", "get-url", "origin"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip() or None
    except Exception:
        return None


def git_repo_identity(remote: str | None) -> tuple[str | None, str | None]:
    """Owner and name parsed from the configured canonical remote or origin."""
    if not remote:
        return None, None
    tail = remote.rstrip("/")
    tail = tail[:-4] if tail.endswith(".git") else tail
    parts = tail.replace(":", "/").split("/")
    return (parts[-2], parts[-1]) if len(parts) >= 2 else (None, None)


def scrub(text: str, secrets: list[str]) -> str:
    for secret in secrets:
        if secret:
            text = text.replace(secret, "<redacted>")
    return text


def call(cfg: dict, path: str, token: str | None, timeout: float) -> tuple[int, dict | str]:
    return acp_client.call(cfg, "GET", path, auth=bool(token), timeout=timeout)


def record_evidence(outcome: str, detail: dict) -> None:
    path = acp_paths.records_dir(ROOT) / "evidence.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    record = {
        "gate": "B0-acp",
        "type": "acp_wiring_check",
        "outcome": outcome,
        "observedAt": datetime.now(timezone.utc).isoformat(),
        "detail": detail,
        "disposition": "Observed",
    }
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(record, sort_keys=True) + "\n")


def onboarding_report(cfg: dict, summary: dict) -> dict:
    """Summarize independent setup states and assign each known blocker."""
    checks = summary.get("checks", {})
    notice = summary.get("notice") or {}
    endpoint_ready = bool(cfg.get("url") and not cfg.get("endpoint_problems"))
    endpoint_names = [item.get("name") for item in cfg.get("endpoints", [])]
    failover_configured = (cfg.get("endpoint_set") != "production" or
                           endpoint_names[:2] == ["vercel", "titan"])
    identity_ready = bool(cfg.get("repo_owner") and cfg.get("repo_name"))
    board_bound = bool(cfg.get("owner") and cfg.get("number"))
    governed = notice.get("governed")
    auth_state = notice.get("authorization_state")
    policy_state = "REGISTERED" if governed is True else "NOT_REGISTERED" if governed is False else "NOT_CHECKED"
    if auth_state in ("QUARANTINED", "LOCKED", "RECOVERY_AUTHORIZED", "VERIFY_RECOVERY"):
        resource_state = auth_state
    elif auth_state == "NORMAL":
        resource_state = ("DISABLED" if notice.get("controlled_operations") == "DISABLED" else
                          "NORMAL" if governed is True and notice.get("controlled_operations") == "PERMITTED" else
                          "NOT_CHECKED")
    elif auth_state is not None:
        resource_state = "UNKNOWN"
    else:
        resource_state = ("DISABLED" if notice.get("controlled_operations") == "DISABLED" else
                          "NOT_CHECKED")

    states = {
        "endpoint_configuration": "INVALID" if cfg.get("endpoint_problems") else
                                  "CONFIGURED" if endpoint_ready else "MISSING",
        "failover_configuration": "CONFIGURED" if failover_configured else "MISSING",
        "endpoint_health": "HEALTHY" if checks.get("health") == "OK" else
                           "FAILED" if checks.get("health") == "FAILED" else "NOT_CHECKED",
        "credential": "ACCEPTED" if checks.get("auth") == "OK" else
                      "REJECTED" if checks.get("auth") == "FAILED" else "NOT_CHECKED",
        "repository_identity": "CONFIGURED" if identity_ready else "MISSING",
        "policy_registration": policy_state,
        "resource_authorization": resource_state,
        "board_binding": "VERIFIED" if checks.get("board") == "OK" else
                         "FAILED" if checks.get("board") == "FAILED" else
                         "CONFIGURED" if board_bound else "MISSING",
    }
    unmet = []
    if states["endpoint_configuration"] != "CONFIGURED":
        unmet.append({"condition": "endpoint_configuration", "owner_role": "project_operator",
                      "next_step": "Select a machine-local endpoint set and configure each endpoint credential in the OS credential store."})
    if states["failover_configuration"] == "MISSING":
        unmet.append({"condition": "failover_configuration", "owner_role": "gateway_operator",
                      "next_step": "Configure production with Vercel first and Titan second, using distinct endpoint credentials and the same verified shared-state identity."})
    if states["endpoint_health"] == "FAILED":
        unmet.append({"condition": "endpoint_health", "owner_role": "gateway_operator",
                      "next_step": "Restore a healthy configured ACP endpoint; retry only after its health check succeeds."})
    elif states["endpoint_health"] == "NOT_CHECKED" and endpoint_ready:
        unmet.append({"condition": "endpoint_health", "owner_role": "gateway_operator",
                      "next_step": "Run the configured endpoint health check before relying on ACP status."})
    if states["credential"] == "REJECTED":
        unmet.append({"condition": "credential", "owner_role": "project_operator",
                      "next_step": "Replace the rejected machine-local bearer with a valid credential through the approved credential path."})
    if states["repository_identity"] == "MISSING":
        unmet.append({"condition": "repository_identity", "owner_role": "project_owner",
                      "next_step": "Set the canonical repository remote or its documented owner/name overrides."})
    if policy_state == "NOT_REGISTERED":
        unmet.append({"condition": "policy_registration", "owner_role": "policy_maintainer",
                      "next_step": "Review the repository policy proposal, register it with ACP, then verify /notice reports it governed."})
    elif policy_state == "NOT_CHECKED":
        unmet.append({"condition": "policy_registration", "owner_role": "project_owner",
                      "next_step": "Run the unauthenticated repository notice check when the configured endpoint is reachable."})
    if resource_state in ("QUARANTINED", "LOCKED"):
        unmet.append({"condition": "policy_authorization", "owner_role": "security_administrator",
                      "next_step": "Review the reported incident and complete authenticated recovery; policy or board configuration cannot clear containment."})
    elif resource_state == "RECOVERY_AUTHORIZED":
        unmet.append({"condition": "policy_authorization", "owner_role": "security_administrator",
                      "next_step": "Complete the authorized recovery procedure, then verify the recovered state before resuming controlled operations."})
    elif resource_state == "VERIFY_RECOVERY":
        unmet.append({"condition": "policy_authorization", "owner_role": "security_administrator",
                      "next_step": "Complete recovery verification with fresh evidence; controlled operations remain unavailable until ACP returns to NORMAL."})
    elif resource_state == "DISABLED":
        unmet.append({"condition": "policy_authorization", "owner_role": "policy_maintainer",
                      "next_step": "Review why controlled operations are disabled and resolve the governing policy or service condition."})
    elif resource_state == "UNKNOWN":
        unmet.append({"condition": "policy_authorization", "owner_role": "gateway_operator",
                      "next_step": "Inspect the unrecognized authorization state and update this client only after confirming the gateway contract."})
    elif resource_state == "NOT_CHECKED" and policy_state == "REGISTERED":
        unmet.append({"condition": "policy_authorization", "owner_role": "project_owner",
                      "next_step": "Read the repository notice and verify authorization_state and controlled_operations before treating the project as ready."})
    if states["board_binding"] == "MISSING":
        unmet.append({"condition": "board_binding", "owner_role": "project_owner",
                      "next_step": "Bind the intended GitHub Project owner and number in the repository board configuration."})
    elif states["board_binding"] == "FAILED":
        unmet.append({"condition": "board_access", "owner_role": "ACP_administrator",
                      "next_step": "Verify the GitHub App installation, Projects permission, and board allowlist for the bound owner and number."})
    return {"states": states, "unmet_conditions": unmet}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--record", action="store_true", help="append the result to the project's evidence ledger")
    parser.add_argument("--timeout", type=float, default=10.0)
    parser.add_argument("--json", action="store_true", help="print the summary as JSON")
    parser.add_argument("--environment", choices=("local", "preview"), default="local",
                        help="local target environment for a time-bounded owner override")
    parser.add_argument("--allow-ungoverned", action="store_true",
                        help="report a repository no ACP policy governs as a warning, not a failure")
    args = parser.parse_args()

    cfg = settings()
    secrets = cfg.get("secrets") or [cfg["token"]]
    summary: dict = {"gateway": cfg["url"], "endpoint_set": cfg["endpoint_set"],
                     "owner": cfg["owner"], "board": cfg["number"], "checks": {}}

    override = acp_override.active(project_id=cfg.get("project_id"), owner=cfg.get("repo_owner"),
                                  repository=cfg.get("repo_name"), branch=cfg.get("branch"),
                                  environment=args.environment)
    if override:
        summary["checks"]["owner_override"] = "ACTIVE"
        summary["owner_override"] = {"override_id": override["override_id"],
                                     "environment": override["environment"],
                                     "branch": override["branch"],
                                     "expires_at": override["expires_at"]}
        record_evidence("OVERRIDE_ACTIVE", summary)
        print(json.dumps(summary, indent=2) if args.json else
              "ACP OWNER OVERRIDE ACTIVE " + json.dumps(summary["owner_override"]))
        return 0

    def fail(code: int, check: str, why, body=None) -> int:
        summary["checks"][check] = "FAILED"
        summary["failure"] = {"check": check, "why": scrub(str(why), secrets)}
        if body is not None:
            summary["failure"]["body"] = json.loads(scrub(json.dumps(body), secrets)) if isinstance(body, dict) else scrub(str(body), secrets)
        summary["onboarding"] = onboarding_report(cfg, summary)
        if args.record:
            record_evidence("FAILED", summary)
        print(json.dumps(summary, indent=2) if args.json else scrub(f"ACP CHECK FAILED at {check}: {why}", secrets), file=sys.stderr)
        if body is not None and not args.json:
            print(scrub(json.dumps(body, indent=2) if isinstance(body, dict) else str(body), secrets), file=sys.stderr)
        return code

    if cfg.get("endpoint_problems"):
        return fail(2, "config", "; ".join(cfg["endpoint_problems"]))
    if not cfg["url"]:
        return fail(2, "config", "the selected machine endpoint set has no endpoint with a URL and its own bearer")
    if not cfg["token"]:
        return fail(2, "config", f"{cfg['token_env']} is not set; LOCAL_GATEWAY_CREDENTIAL_MISSING is a setup blocker, not a gateway result")
    if cfg["identity_error"]:
        return fail(2, "config", cfg["identity_error"])
    summary["checks"]["config"] = "OK"

    notice_failure = None
    try:
        status, body = call(cfg, "/health", None, args.timeout)
        if status != 200 or not isinstance(body, dict) or body.get("status") != "ok":
            return fail(3, "health", f"HTTP {status}", body)
        summary["checks"]["health"] = "OK"
        summary["app_configured"] = bool(body.get("app_configured"))

        if cfg["repo_owner"] and cfg["repo_name"]:
            resource = f"{cfg['repo_owner']}/{cfg['repo_name']}"
            status, body = call(
                cfg,
                "/notice?" + urlencode({"owner": cfg["repo_owner"], "name": cfg["repo_name"]}),
                None,
                args.timeout,
            )
            if status != 200 or not isinstance(body, dict):
                return fail(3, "notice", f"HTTP {status}", body)
            summary["notice"] = {k: body.get(k) for k in ("governed", "authorization_state", "policy_id", "policy_version", "controlled_operations", "incident_id", "integrity", "gate_mode", "gate_status", "gate_blocked", "active_block", "error") if k in body}
            if body.get("governed") is not True:
                why = (f"no ACP policy governs {resource}: inside ACP_POLICY_SCOPE every protected action is "
                       "refused (POLICY_NOT_FOUND); outside it ACP answers ALLOW/NOT_GOVERNED, which "
                       ".agents/acp/acp refuses while the profile is failClosed. The repository owner registers "
                       "one on the gateway: scripts/policy-admin.py register OWNER/NAME --apply")
                if cfg["fail_closed"] and not args.allow_ungoverned:
                    notice_failure = why
                    summary["checks"]["notice"] = "FAILED (not governed)"
                else:
                    summary["checks"]["notice"] = "WARNING (not governed)"
                    summary["warning"] = why
            else:
                summary["checks"]["notice"] = "OK"
        else:
            summary["checks"]["notice"] = "SKIPPED (repository owner or name unknown)"

        status, body = call(cfg, "/internal/app-status", cfg["token"], args.timeout)
        if status == 401:
            return fail(3, "auth", "gateway rejected the bearer token", body)
        if status != 200:
            return fail(3, "auth", f"HTTP {status}", body)
        summary["checks"]["auth"] = "OK"

        if cfg["owner"]:
            status, body = call(cfg, "/internal/projects?" + urlencode({"owner": cfg["owner"]}), cfg["token"], args.timeout)
            if status != 200 or not isinstance(body, dict):
                return fail(3, "owner", f"HTTP {status}", body)
            summary["checks"]["owner"] = "OK"
            boards = body.get("projects") or body.get("boards") or []
            summary["boards_visible"] = len(boards)
            if cfg["number"]:
                match = next((b for b in boards if str(b.get("number")) == cfg["number"]), None)
                if match is None:
                    return fail(3, "board", f"board #{cfg['number']} of {cfg['owner']} is not visible to the gateway", body)
                if match.get("allowed") is False:
                    return fail(3, "board", f"board #{cfg['number']} is outside ACP_PROJECT_ALLOWLIST", match)
                summary["checks"]["board"] = "OK"
                summary["board_title"] = match.get("title")
            else:
                summary["checks"]["board"] = "SKIPPED (no board bound)"
        else:
            summary["checks"]["owner"] = "SKIPPED (no owner bound)"
            summary["checks"]["board"] = "SKIPPED (no owner bound)"

    except (ConnectionError, acp_client.Unreachable) as exc:
        return fail(4, "reach", f"gateway unreachable: {exc}")

    if cfg.get("active_endpoint"):
        summary["selected_endpoint"] = cfg["active_endpoint"]
        summary["gateway"] = next((item["url"] for item in cfg["endpoints"]
                                   if item["name"] == cfg["active_endpoint"]), summary["gateway"])
    if notice_failure:
        return fail(5, "notice", notice_failure)
    summary["onboarding"] = onboarding_report(cfg, summary)
    if args.record:
        record_evidence("OK", summary)
    print(json.dumps(summary, indent=2) if args.json else "ACP CHECK OK " + json.dumps(summary["checks"]))
    if summary.get("warning") and not args.json:
        print("WARNING: " + summary["warning"], file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
