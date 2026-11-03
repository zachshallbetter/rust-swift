#!/usr/bin/env python3
"""acp-gateway client for a formal-project-bootstrap project. Standard library only.

    acp.py authorize --action git.protected_ref.update --ref refs/heads/main --old-sha X --new-sha Y
    acp.py report    --event SESSION_STARTED | PROTECTED_EFFECT_PENDING --action A | REMOTE_CHANGED ...
    acp.py snapshot  --checkpoint SESSION_START [--path P ...]
    acp.py verify-capability  --token T      (or --decision decision.json)
    acp.py recovery-complete  --token T --result succeeded|failed --observed-sha S
    acp.py notice              is this repository governed? (exit 5 if not; unauthenticated)
    acp.py board               read the bound ProjectsV2 board through ACP
    acp.py policy              emit this repository's candidate ACP policy (for register-policy.py)

The decision rules live in docs/ACP_INTEGRATION.md; this client applies the
ones a script can apply without judgement:

  * fail closed -- unreachable, refused, malformed or out-of-vocabulary output
    becomes DENY / GATEWAY_UNAVAILABLE (synthetic: true), never a pass;
  * NOT_GOVERNED is not ALLOW -- a governed action that ACP answers with
    policy_effect NOT_GOVERNED (no policy names this repository) is DENY /
    ACP_NOT_GOVERNING while the profile is failClosed;
  * exact action -- an ALLOW whose authorized_action differs from what was
    asked is DENY / AUTHORIZED_ACTION_MISMATCH.

Exit codes for `authorize`: 0 ALLOW, 11 DENY, 12 AUTH_REQUIRED,
13 REVERIFY_REQUIRED, 14 QUARANTINE, 15 LOCKED, 16 RECOVERY_AUTHORIZED,
17 VERIFY_RECOVERY, 2 local configuration missing. Every other command:
0 accepted, 2 configuration, 3 gateway refused, 4 unreachable, 5 not governed.

The gateway token is read from the environment or .env.local, then from the
owner's macOS Keychain when available; it is never printed or recorded. The
admin credential is never loaded from Keychain implicitly. A capability_token
is printed (the next step needs it) but never written to records/.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

import acp_override
import acp_endpoint_registry

ROOT = Path(__file__).resolve().parents[1]
VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip() if (ROOT / "VERSION").exists() else "unknown"
PROJECT_ROOT = Path.cwd().resolve()
KEYCHAIN_SERVICE = "ACP Gateway Production"


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


_HTTP = build_opener(_NoRedirect())
MAX_HTTP_RESPONSE_BYTES = 8 * 1024 * 1024
# A healthy serverless /ready answers in ~1.3-1.6s warm and slower from a cold start; a tighter
# limit turns a slow but ready endpoint into "no endpoint reports readiness".
READINESS_PROBE_TIMEOUT = 8.0
TRANSIENT_READ_ERRORS = frozenset({"UPSTREAM_FAILURE", "TEMPORARY", "SERVICE_UNAVAILABLE"})
HARD_STOP_READINESS_ERRORS = frozenset({"APP_NOT_CONFIGURED", "POLICY_INVALID", "SECURITY_STATE_UNAVAILABLE",
                                       "STATE_STORE_UNAVAILABLE", "INTEGRITY_INVALID", "INTEGRITY_MISMATCH",
                                       "REVERIFY_REQUIRED", "QUARANTINED", "LOCKED", "VERIFY_RECOVERY"})

DECISIONS = ("ALLOW", "DENY", "AUTH_REQUIRED", "REVERIFY_REQUIRED", "QUARANTINE",
             "LOCKED", "RECOVERY_AUTHORIZED", "VERIFY_RECOVERY")
EXIT = {"ALLOW": 0, "DENY": 11, "AUTH_REQUIRED": 12, "REVERIFY_REQUIRED": 13, "QUARANTINE": 14,
        "LOCKED": 15, "RECOVERY_AUTHORIZED": 16, "VERIFY_RECOVERY": 17}
# E0: never governed, so ACP's answer for them is informational.
LOCAL_ACTIONS = acp_override.LOCAL_ACTIONS
ADMIN_ACTIONS = frozenset({"policy.modify", "credential.modify"})
REQUIRED = ("decision", "reason", "action", "request_id", "evaluated_at")
CHECKPOINTS = ("SESSION_START", "SESSION_RESUME", "PRE_CONTROLLED_OPERATION", "PRE_COMMIT",
               "PRE_PUSH", "PRE_MERGE", "PRE_DEPLOY", "POST_RECOVERY")
# The bootstrap's own control surface: what an agent would edit to talk itself
# out of a decision. Hashed for snapshots, and proposed as the policy manifest.
# (path, kind, required). The change policy is chosen at `policy` time:
# authorize_and_report by default, admin_exact_transition with --strict.
CONTROL_ARTIFACTS = (
    ("AGENTS.md", "control_instruction", True),
    ("NEW_AGENT_PROMPT.md", "control_instruction", False),
    ("PROJECT_PROFILE.json", "bootstrap", True),
    ("PROJECT_INTENT.md", "bootstrap", True),
    ("bindings/PROJECT_BINDINGS.yaml", "bootstrap", True),
    ("contracts/acp-protected-effects.yaml", "policy_manifest", True),
    ("docs/ACP_INTEGRATION.md", "control_instruction", True),
    (".agents/board.env", "repository_identity", False),
    ("scripts/acp.py", "client_control", True),
    ("scripts/acp_endpoint_registry.py", "client_control", True),
    ("schemas/acp-endpoint-registry.schema.json", "schema", True),
    ("scripts/acp-check.py", "client_control", True),
    ("scripts/acp_override.py", "client_control", True),
    ("scripts/upgrade-project.py", "client_control", True),
    ("scripts/reconcile-project.py", "client_control", True),
)
# Trees whose every file is governed without being pinned by digest.
CONTROL_PATTERNS = (
    (".agents/skills/**", "client_control_tree", "authorize_and_report"),
    ("records/deviations.jsonl", "control_tree", "report_only"),
)


# ------------------------------------------------------------------ settings

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


def git(*args: str) -> str | None:
    try:
        return subprocess.check_output(["git", "-C", str(PROJECT_ROOT), *args], text=True,
                                       stderr=subprocess.DEVNULL).strip() or None
    except Exception:
        return None


def keychain_secret(account: str, enabled: bool) -> str:
    """Read the gateway bearer from the owner's macOS Keychain without printing it."""
    if (not enabled or platform.system() != "Darwin"
            or not (account == "ACP_GATEWAY_TOKEN" or account.startswith("ACP_GATEWAY_TOKEN_"))):
        return ""
    try:
        result = subprocess.run(
            ["security", "find-generic-password", "-s", KEYCHAIN_SERVICE,
             "-a", account, "-w"],
            capture_output=True, text=True, timeout=3, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return ""
    value = result.stdout.rstrip("\r\n") if result.returncode == 0 else ""
    if not value or len(value) > 4096 or any(char.isspace() for char in value):
        return ""
    return value


def owner_and_name(remote: str | None) -> tuple[str | None, str | None]:
    if not remote:
        return None, None
    tail = remote.rstrip("/")
    if tail.endswith(".git"):
        tail = tail[:-4]
    parts = tail.replace(":", "/").split("/")
    return (parts[-2], parts[-1]) if len(parts) >= 2 else (None, None)


def settings() -> dict:
    profile = {}
    if (PROJECT_ROOT / "PROJECT_PROFILE.json").exists():
        profile = json.loads((PROJECT_ROOT / "PROJECT_PROFILE.json").read_text(encoding="utf-8"))
    acp = profile.get("authorizationProvider", {}) or {}
    board_env_file = acp.get("boardEnvFile", ".agents/board.env")
    board_env = load_env_file(PROJECT_ROOT / board_env_file)
    local_env = load_env_file(PROJECT_ROOT / ".env.local")
    process_env = {k: v for k, v in os.environ.items() if k.startswith(("ACP_", "GH_PROJECT_"))}
    env: dict[str, str] = {**board_env, **local_env, **process_env}
    board = acp.get("board") or {}
    remote = env.get("ACP_REMOTE_URL") or git("remote", "get-url", "origin")
    remote_owner, remote_name = owner_and_name(remote)
    keychain_enabled = env.get("ACP_KEYCHAIN_LOOKUP", "1").lower() not in ("0", "false", "no")
    endpoint_set = ""
    endpoint_specs = []
    endpoint_problems = []
    machine_registry_path = None
    try:
        machine_registry_path = str(acp_endpoint_registry.registry_path(env))
    except acp_endpoint_registry.EndpointRegistryError as exc:
        endpoint_problems.append(str(exc))
    try:
        endpoint_set, endpoint_specs = acp_endpoint_registry.resolve(acp, env)
    except acp_endpoint_registry.EndpointRegistryError as exc:
        endpoint_problems.append(str(exc))
    endpoints = []
    missing_accounts = []
    admin_token = env.get(acp.get("adminTokenEnv", "ACP_POLICY_ADMIN_TOKEN"), "")
    for index, spec in enumerate(endpoint_specs):
        account = spec["keychain_account"]
        token = env.get(account) or keychain_secret(account, keychain_enabled)
        if not token:
            missing_accounts.append(account)
            continue
        if process_env.get(account):
            token_source = f"environment variable {account}"
        elif local_env.get(account):
            token_source = f".env.local {account}"
        elif board_env.get(account):
            token_source = f"{board_env_file} {account}"
        else:
            token_source = f"Keychain service '{KEYCHAIN_SERVICE}', account {account}"
        endpoints.append({"name": spec["name"], "url": spec["url"], "token": token,
                          "token_env": account, "token_source": token_source, "local_only": spec["local_only"],
                          "state_authority_id": spec.get("state_authority_id"),
                          "admin_token": admin_token if index == 0 else ""})
    if endpoint_specs and not endpoints and not endpoint_problems:
        endpoint_problems.append("no endpoint in the selected set has its own bearer; configure Keychain accounts: "
                                 + ", ".join(missing_accounts))
    owner_override = env.get("ACP_REPOSITORY_OWNER")
    name_override = env.get("ACP_REPOSITORY_NAME")
    identity_error = None
    if remote:
        if owner_override and remote_owner and owner_override.lower() != remote_owner.lower():
            identity_error = "ACP_REPOSITORY_OWNER conflicts with ACP_REMOTE_URL or origin"
        elif name_override and remote_name and name_override.lower() != remote_name.lower():
            identity_error = "ACP_REPOSITORY_NAME conflicts with ACP_REMOTE_URL or origin"
        owner, name = remote_owner, remote_name
    else:
        owner, name = owner_override, name_override
    return {
        "url": endpoints[0]["url"] if endpoints else "",
        "token": endpoints[0]["token"] if endpoints else "",
        "endpoints": endpoints,
        "endpoint_problems": endpoint_problems,
        "secrets": [secret for endpoint in endpoints for secret in
                    (endpoint["token"], endpoint["admin_token"]) if secret],
        "token_env": endpoints[0]["token_env"] if endpoints else
                     (endpoint_specs[0]["keychain_account"] if endpoint_specs else ""),
        "endpoint_set": endpoint_set,
        "machine_registry_path": machine_registry_path,
        "admin_token": next((endpoint["admin_token"] for endpoint in endpoints
                             if endpoint["admin_token"]), ""),
        "fail_closed": acp.get("failClosed", True) is not False,
        "board_owner": env.get("ACP_BOARD_OWNER") or env.get("GH_PROJECT_OWNER") or board.get("owner"),
        "board_number": env.get("ACP_BOARD_NUMBER") or env.get("GH_PROJECT_NUMBER") or board.get("number"),
        "owner": env.get("ACP_REPOSITORY_OWNER") or owner,
        "name": name,
        "identity_error": identity_error,
        "remote": remote,
        "agent_id": env.get("ACP_AGENT_ID") or "fpb-agent",
        "actor_id": env.get("ACP_ACTOR_ID"),
        "session_id": env.get("ACP_SESSION_ID"),
        "project_id": profile.get("projectId"),
        "project_root": str(PROJECT_ROOT),
        "branch": git("symbolic-ref", "--quiet", "--short", "HEAD"),
        "auto_deploy": auto_deploy_rules(acp),
    }


# Files that show a hosting integration deploys this repository on push.
DEPLOY_INDICATORS = ("vercel.json", ".vercel/project.json", "netlify.toml")


def auto_deploy_rules(acp: dict) -> list[dict]:
    """authorizationProvider.autoDeploy: refs whose update also deploys an environment.

    Each entry is {"ref": "refs/heads/main", "environment": "production"}, plus
    "decision": "merge" where the owner has decided that the ref update is the
    deploy decision; otherwise the client also asks for production.deploy.
    """
    rules = []
    for entry in acp.get("autoDeploy") or []:
        if not isinstance(entry, dict) or not str(entry.get("ref", "")).startswith("refs/"):
            continue
        rules.append({"ref": entry["ref"], "environment": entry.get("environment", "production"),
                      "decision": "merge" if entry.get("decision") == "merge" else "separate"})
    return rules


def scrub(text: str, cfg: dict) -> str:
    for secret in set(cfg.get("secrets", ())) | {cfg.get("token"), cfg.get("admin_token")}:
        if secret:
            text = text.replace(secret, "<redacted>")
    return text


# ------------------------------------------------------------------ transport

class Unreachable(Exception):
    pass


class ResponseRejected(Unreachable):
    """A protocol or integrity failure that must not be hidden by failover."""


def _http_call(endpoint: dict, method: str, path: str, body: dict | None = None,
               auth: bool = True, admin_auth: bool = False,
               timeout: float = 15.0, idempotency_key: str | None = None) -> tuple[int, object]:
    headers = {"Accept": "application/json", "User-Agent": f"fpb-acp/{VERSION}"}
    data = None
    if body is not None:
        data = json.dumps(body, separators=(",", ":")).encode()
        if len(data) > MAX_HTTP_RESPONSE_BYTES:
            raise ResponseRejected("request exceeds the 8 MiB ACP client limit")
        headers["Content-Type"] = "application/json"
    if auth:
        headers["Authorization"] = "Bearer " + endpoint["token"]
    if admin_auth:
        headers["X-ACP-Admin-Authorization"] = "Bearer " + endpoint["admin_token"]
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    request = Request(endpoint["url"] + path, data=data, method=method, headers=headers)
    try:
        with _HTTP.open(request, timeout=timeout) as response:
            status, raw = response.status, response.read(MAX_HTTP_RESPONSE_BYTES + 1)
    except HTTPError as exc:
        status, raw = exc.code, exc.read(MAX_HTTP_RESPONSE_BYTES + 1)
    except (URLError, TimeoutError, OSError) as exc:
        raise Unreachable(str(getattr(exc, "reason", exc))) from exc
    if len(raw) > MAX_HTTP_RESPONSE_BYTES:
        raise ResponseRejected("ACP response exceeds the 8 MiB client limit")
    raw = raw.decode("utf-8", "replace")
    try:
        return status, json.loads(raw)
    except json.JSONDecodeError:
        return status, raw


def select_endpoint(cfg: dict, timeout: float = 15.0, admin: bool = False,
                    exclude: set[str] | None = None) -> dict:
    deadline = time.monotonic() + timeout
    exclude = exclude or set()
    endpoints = [item for item in cfg.get("endpoints", [])
                 if item["name"] not in exclude and (not admin or item.get("admin_token"))]
    if not endpoints:
        raise Unreachable("no configured ACP endpoint has the required credentials")
    failures = []
    for endpoint in endpoints:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            failures.append("readiness deadline exhausted before remaining endpoints were probed")
            break
        probe = f"{endpoint['name']} /ready"
        limit = min(remaining, READINESS_PROBE_TIMEOUT)
        started = time.monotonic()
        try:
            status, body = _http_call(endpoint, "GET", "/ready", auth=False, timeout=limit)
        except ResponseRejected:
            raise
        except Unreachable:
            elapsed = round((time.monotonic() - started) * 1000)
            hint = (f" (probe limit {round(limit * 1000)}ms; the endpoint may be cold-starting, retry or raise --timeout)"
                    if elapsed >= limit * 1000 * 0.9 else "")
            failures.append(f"{probe}: transport unavailable after {elapsed}ms{hint}")
            continue
        if (status == 200 and isinstance(body, dict) and body.get("status") == "ready"
                and body.get("storage") == "postgres"):
            expected_state_id = endpoint.get("state_authority_id")
            if expected_state_id and body.get("state_authority_id") != expected_state_id:
                raise ResponseRejected(
                    f"ACP endpoint {probe} HTTP {status}: different or missing shared state authority; "
                    "ACP operator must verify endpoint identity; failover stopped")
            capability = body.get("idempotency")
            endpoint["idempotency_routes"] = (
                set(capability["routes"])
                if isinstance(capability, dict) and capability.get("version") == 1
                and (not expected_state_id or capability.get("state_authority_id") == expected_state_id)
                and isinstance(capability.get("routes"), list)
                and all(isinstance(route, str) and route.startswith("/")
                        for route in capability["routes"])
                and len(set(capability["routes"])) == len(capability["routes"])
                else set()
            )
            cfg["active_endpoint"] = endpoint["name"]
            return endpoint
        error_code = body.get("error") if isinstance(body, dict) else None
        if isinstance(error_code, dict):
            error_code = error_code.get("code")
        if status in (401, 403) or (isinstance(body, dict) and
                             (body.get("decision") in DECISIONS
                              or error_code in HARD_STOP_READINESS_ERRORS)):
            raise ResponseRejected(f"ACP endpoint {probe} HTTP {status}: rejected readiness or reported a control failure; "
                                   "ACP operator must inspect credentials, policy, or containment; failover stopped")
        if status >= 500 and (not isinstance(body, dict)
                              or body.get("error") not in TRANSIENT_READ_ERRORS):
            raise ResponseRejected(f"ACP endpoint {probe} HTTP {status}: unclassified readiness failure; "
                                   "ACP operator must inspect runtime and storage logs; failover stopped")
        elapsed = round((time.monotonic() - started) * 1000)
        shape = "JSON object not ready" if isinstance(body, dict) else "non-object or non-JSON response"
        failures.append(f"{probe}: HTTP {status}, {shape}, {elapsed}ms")
    raise Unreachable("no configured ACP endpoint reports PostgreSQL readiness ("
                      + "; ".join(failures) + "). ACP operator must inspect the named readiness probes; "
                      "local E0/E1 work may continue, protected effects remain blocked")


def call(cfg: dict, method: str, path: str, body: dict | None = None, auth: bool = True,
         timeout: float = 15.0) -> tuple[int, object]:
    deadline = time.monotonic() + timeout
    endpoint = select_endpoint(cfg, timeout=max(0.01, deadline - time.monotonic()))
    method = method.upper()
    idempotency_key = (uuid.uuid4().hex
                       if method == "POST" and path in endpoint.get("idempotency_routes", set()) else None)
    excluded = set()
    while True:
        try:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Unreachable("ACP request deadline exceeded")
            result = _http_call(endpoint, method, path, body, auth=auth, timeout=remaining,
                                idempotency_key=idempotency_key)
        except ResponseRejected:
            raise
        except Unreachable:
            if method != "GET" and not idempotency_key:
                raise
            excluded.add(endpoint["name"])
            if len(excluded) >= len(cfg.get("endpoints", [])):
                raise
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Unreachable("ACP request deadline exceeded")
            endpoint = select_endpoint(cfg, timeout=remaining, exclude=excluded)
            continue
        status, reply = result
        if auth and status == 401 and isinstance(reply, dict):
            source = endpoint.get("token_source", "the configured bearer")
            reply = {**reply, "client_credential_source": source,
                     "client_remedy": (f"ACP answered and rejected the bearer read from {source}. Update that "
                                       "credential to the current gateway token; this is not a gateway outage.")}
            result = (status, reply)
        transient = (status in (502, 503, 504) and isinstance(reply, dict)
                     and reply.get("decision") not in DECISIONS
                     and reply.get("error") in TRANSIENT_READ_ERRORS)
        if (method != "GET" and not (method == "POST" and idempotency_key and transient)) or not transient:
            return result
        excluded.add(endpoint["name"])
        if len(excluded) >= len(cfg.get("endpoints", [])):
            return result
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return result
        endpoint = select_endpoint(cfg, timeout=remaining, exclude=excluded)


def call_admin(cfg: dict, path: str, body: dict, timeout: float = 15.0) -> tuple[int, object]:
    if not cfg.get("endpoints") or not any(item.get("admin_token") for item in cfg["endpoints"]):
        raise ValueError("ACP administrative operations require ACP_GATEWAY_TOKEN and ACP_POLICY_ADMIN_TOKEN")
    deadline = time.monotonic() + timeout
    endpoint = select_endpoint(cfg, timeout=max(0.01, deadline - time.monotonic()), admin=True)
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise Unreachable("ACP request deadline exceeded")
    return _http_call(endpoint, "POST", path, body, admin_auth=True, timeout=remaining)


def principal(cfg: dict) -> dict:
    out = {"agent_id": cfg["agent_id"], "client_type": "formal-project-bootstrap",
           "client_version": VERSION}
    if cfg["actor_id"]:
        out["actor_id"] = cfg["actor_id"]
    if cfg["session_id"]:
        out["session_id"] = cfg["session_id"]
    return out


def resource(cfg: dict, ref: str | None = None) -> dict:
    out = {k: v for k, v in (("owner", cfg["owner"]), ("name", cfg["name"]),
                              ("remote_url", cfg["remote"]), ("ref", ref)) if v}
    return out


def require(cfg: dict, repository: bool = False) -> str | None:
    if cfg.get("identity_error"):
        return cfg["identity_error"]
    if cfg.get("endpoint_problems"):
        return "; ".join(cfg["endpoint_problems"])
    if not cfg.get("endpoints"):
        return "no ACP endpoint has a URL and its own bearer; configure the selected machine endpoint set"
    if repository and not (cfg["owner"] and cfg["name"]):
        return "repository owner/name unknown: set an origin remote or ACP_REPOSITORY_OWNER/NAME"
    return None


def emit(value: object, cfg: dict, stream=sys.stdout) -> None:
    print(scrub(json.dumps(value, indent=2, sort_keys=True), cfg), file=stream)


def record_evidence(entry: dict) -> None:
    path = PROJECT_ROOT / "records" / "evidence.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    entry = {k: v for k, v in entry.items() if k not in ("capability_token", "capability")}
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry, sort_keys=True) + "\n")


# ------------------------------------------------------------------ authorize

def synthetic(action: str, reason: str, detail: str, **extra) -> dict:
    return {"decision": "DENY", "code": "A1", "reason": reason, "action": action,
            "request_id": "local-" + uuid.uuid4().hex[:16],
            "evaluated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "detail": detail, "synthetic": True, **extra}


def dispose(action: str, asked: dict, status: int, body: object, fail_closed: bool) -> dict:
    """Turn whatever came back into one decision, failing closed."""
    if not isinstance(body, dict):
        return synthetic(action, "GATEWAY_UNAVAILABLE", f"HTTP {status}: response was not JSON")
    if status == 401 and body.get("client_credential_source"):
        return synthetic(action, "GATEWAY_UNAVAILABLE",
                         f"HTTP 401: {body.get('error')}: {body.get('detail')} {body['client_remedy']}",
                         gateway_error=body.get("error"), remedy=body["client_remedy"],
                         credential_rejected=True, credential_source=body["client_credential_source"])
    if status != 200:
        return synthetic(action, "GATEWAY_UNAVAILABLE",
                         f"HTTP {status}: {body.get('error')}: {body.get('detail')}",
                         gateway_error=body.get("error"), remedy=body.get("remedy"))
    missing = [field for field in REQUIRED if not body.get(field)]
    if missing or body.get("decision") not in DECISIONS:
        return synthetic(action, "GATEWAY_UNAVAILABLE",
                         f"malformed decision (missing {missing or 'valid decision'})")
    if body["decision"] != "ALLOW" or action in LOCAL_ACTIONS:
        return body
    if body.get("policy_effect") == "NOT_GOVERNED":
        if not fail_closed:
            return body
        return synthetic(action, "ACP_NOT_GOVERNING",
                         "ACP answered ALLOW because no policy governs this repository "
                         f"({body.get('reason')}). The profile is fail-closed, so that is not an "
                         "authorization. Register one: acp.py policy > p.json; register-policy.py --from p.json.",
                         gateway_decision={k: body.get(k) for k in ("decision", "reason", "policy_effect",
                                                                    "request_id")})
    granted = body.get("authorized_action") or {}
    for field in ("action", "ref", "old_sha", "new_sha"):
        wanted = action if field == "action" else asked.get(field)
        if wanted is not None and granted.get(field) != wanted:
            return synthetic(action, "AUTHORIZED_ACTION_MISMATCH",
                             f"ALLOW authorized {field}={granted.get(field)!r}, not {wanted!r}",
                             gateway_decision={k: body.get(k) for k in ("decision", "reason", "request_id")})
    if action == "database.migrate":
        for field in ("source_database", "target_database", "work_item", "environment", "ref", "new_sha"):
            if granted.get(field) != asked.get(field):
                return synthetic(action, "AUTHORIZED_ACTION_MISMATCH",
                                 f"ALLOW authorized {field}={granted.get(field)!r}, not {asked.get(field)!r}",
                                 gateway_decision={k: body.get(k) for k in ("decision", "reason", "request_id")})
    return body


def cmd_authorize(args, cfg) -> int:
    problem = require(cfg, repository=True)
    admin_action = args.action in ADMIN_ACTIONS
    if admin_action and not cfg.get("admin_token"):
        print("ACP_POLICY_ADMIN_TOKEN is required for this administrative action.", file=sys.stderr)
        return 2
    environment = args.environment or acp_override.inferred_environment(args.action)
    if not acp_override.valid_action_environment(args.action, environment):
        print(f"{args.action} cannot be authorized in environment {environment}.", file=sys.stderr)
        return 2
    context = {k: v for k, v in (("ref", args.ref), ("old_sha", args.old_sha),
                                 ("new_sha", args.new_sha)) if v}
    if args.action == "database.migrate":
        if not args.context_json or not args.work_item or not args.ref or not args.new_sha:
            print("database.migrate requires --context-json with source_database and target_database, --work-item, --ref, and --new-sha",
                  file=sys.stderr)
            return 2
        try:
            extra = json.loads(args.context_json)
        except ValueError:
            print("--context-json must be a JSON object; input omitted", file=sys.stderr)
            return 2
        if not isinstance(extra, dict) or set(extra) != {"source_database", "target_database"}:
            print("database.migrate context must contain exactly source_database and target_database",
                  file=sys.stderr)
            return 2
        if not all(isinstance(extra[name], str) and extra[name].strip()
                   for name in ("source_database", "target_database")):
            print("database.migrate identifiers must be non-empty strings", file=sys.stderr)
            return 2
        if extra["source_database"] == extra["target_database"]:
            print("database.migrate source and target identifiers must differ", file=sys.stderr)
            return 2
        if any("@" in value or "://" in value or any(term in value.lower()
                   for term in ("password=", "secret=", "token="))
               for value in extra.values()):
            print("database.migrate identifiers must not contain credentials or URLs", file=sys.stderr)
            return 2
        context.update(extra)
        context["work_item"] = args.work_item
        context["environment"] = environment
    elif args.context_json:
        print("--context-json is currently reserved for database.migrate", file=sys.stderr)
        return 2
    # The deployed ACP API does not accept environment on ordinary decisions.
    # Keep sending it only when required to scope a shared owner override.
    if args.owner_override_id:
        context["environment"] = environment
    if args.force:
        context["force"] = True
    if args.recovery_authorization_id:
        context["recovery_authorization_id"] = args.recovery_authorization_id
    if args.owner_override_id:
        context["owner_override_id"] = args.owner_override_id
    def decide(action: str, context: dict) -> dict:
        if problem:
            return synthetic(action, "GATEWAY_UNAVAILABLE", problem, local_setup=True)
        claim = {k: v for k, v in (("owner", cfg["owner"]), ("name", cfg["name"]),
                                   ("remote_url", cfg["remote"]),
                                   ("head_sha", git("rev-parse", "HEAD"))) if v}
        request = {"action": action, "agent_id": cfg["agent_id"], "repository_claim": claim,
                   "context": context,
                   "principal": {k: v for k, v in principal(cfg).items()
                                 if k in ("client_type", "client_version")}}
        for key in ("actor_id", "session_id"):
            if cfg[key]:
                request[key] = cfg[key]
        if args.summary:
            request["request_provenance"] = {"source": args.provenance, "request_summary": args.summary}
        try:
            if action in ADMIN_ACTIONS:
                status, body = call_admin(cfg, "/internal/authorize", request, args.timeout)
            else:
                status, body = call(cfg, "POST", "/internal/authorize", request, timeout=args.timeout)
            return dispose(action, context, status, body, cfg["fail_closed"])
        except Unreachable as exc:
            return synthetic(action, "GATEWAY_UNAVAILABLE", f"unreachable: {exc}")

    def record(decision: dict, context: dict) -> None:
        record_evidence({"gate": "acp-decision", "type": "acp_authorization", "disposition": "Observed",
                         "observedAt": datetime.now(timezone.utc).isoformat(),
                         "workItem": args.work_item, "context": context,
                         "gateway_endpoint": cfg.get("active_endpoint"),
                         "gateway_endpoint_set": cfg.get("endpoint_set"),
                         **{k: decision.get(k) for k in ("decision", "code", "reason", "action", "request_id",
                                                         "policy_id", "policy_version", "repository_trust",
                                                         "authorization_state", "incident_id", "synthetic")}})

    decision = decide(args.action, context)
    if args.record:
        record(decision, context)
    exit_code = 2 if problem else EXIT.get(decision["decision"], 11)
    if args.action == "git.protected_ref.update" and decision["decision"] == "ALLOW":
        rule = next((r for r in cfg["auto_deploy"] if r["ref"] == args.ref), None)
        if rule and rule["decision"] == "separate":
            # Moving this ref deploys production, so the update is only safe
            # to perform when the deploy is authorized too.
            deploy_context = {k: v for k, v in context.items() if k in ("ref", "old_sha", "new_sha")}
            deploy = decide("production.deploy", deploy_context)
            if args.record:
                record(deploy, deploy_context)
            decision = {**decision, "deploy_decision": deploy}
            if deploy["decision"] != "ALLOW":
                print(f"{args.ref} deploys {rule['environment']} automatically, and production.deploy was "
                      f"{deploy['decision']} ({deploy.get('reason')}): do not update the ref.", file=sys.stderr)
                exit_code = EXIT.get(deploy["decision"], 11)
        elif not rule:
            found = [name for name in DEPLOY_INDICATORS if (PROJECT_ROOT / name).exists()]
            if found and args.ref in ("refs/heads/main", "refs/heads/master"):
                print(f"warning: {', '.join(found)} suggests {args.ref} deploys automatically, so updating it "
                      "may also deploy production. Declare authorizationProvider.autoDeploy in "
                      "PROJECT_PROFILE.json so the client asks for production.deploy too.", file=sys.stderr)
    emit(decision, cfg)
    return exit_code


# ------------------------------------------------------------------ others

def post(cfg, path, body, args) -> int:
    try:
        status, reply = call(cfg, "POST", path, body, timeout=args.timeout)
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4
    emit(reply, cfg, sys.stdout if status == 200 else sys.stderr)
    return 0 if status == 200 else 3


def cmd_report(args, cfg) -> int:
    problem = require(cfg, repository=True)
    if problem:
        print(problem, file=sys.stderr)
        return 2
    evidence = {k: v for k, v in (("checkpoint", args.checkpoint), ("action", args.action),
                                  ("detail", args.detail)) if v}
    if args.path:                                   # the file the event is about, with its digest
        evidence["path"] = args.path
        seen = observe(args.path)
        if seen.get("sha256"):
            evidence["current_sha256"] = seen["sha256"]
    body = {"event_type": args.event, "event_id": "fpb-" + uuid.uuid4().hex[:16],
            "observed_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "principal": principal(cfg), "resource": resource(cfg, args.ref)}
    if evidence:
        body["evidence"] = evidence
    return post(cfg, "/internal/report", body, args)


def observe(path: str) -> dict:
    target = PROJECT_ROOT / path
    if target.is_symlink():
        return {"path": path, "state": "present", "symlink": True}
    if not target.exists():
        return {"path": path, "state": "missing"}
    try:
        data = target.read_bytes()
    except OSError:
        return {"path": path, "state": "unreadable"}
    return {"path": path, "state": "present", "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


def cmd_snapshot(args, cfg) -> int:
    problem = require(cfg, repository=True)
    if problem:
        print(problem, file=sys.stderr)
        return 2
    paths = args.path or [path for path, _, _ in CONTROL_ARTIFACTS]
    repository = {k: v for k, v in (("remote_url", cfg["remote"]), ("head_sha", git("rev-parse", "HEAD")),
                                    ("branch", git("rev-parse", "--abbrev-ref", "HEAD"))) if v}
    status = git("status", "--porcelain")
    repository["dirty"] = bool(status)
    body = {"checkpoint": args.checkpoint, "principal": principal(cfg), "resource": resource(cfg),
            "repository": repository, "artifacts": [observe(p) for p in paths]}
    return post(cfg, "/internal/integrity/snapshot", body, args)


def token_from(args) -> str | None:
    if args.token:
        return args.token
    if args.decision:
        source = sys.stdin if args.decision == "-" else open(args.decision, encoding="utf-8")
        with source:
            return (json.load(source) or {}).get("capability_token")
    return None


def cmd_notice(args, cfg) -> int:
    """Is this repository governed? Unauthenticated, like the gateway endpoint."""
    if cfg.get("identity_error"):
        print(cfg["identity_error"], file=sys.stderr)
        return 2
    if cfg.get("endpoint_problems"):
        print("; ".join(cfg["endpoint_problems"]), file=sys.stderr)
        return 2
    if not cfg["url"]:
        print("the selected ACP endpoint set has no configured URL and Keychain bearer", file=sys.stderr)
        return 2
    if not (cfg["owner"] and cfg["name"]):
        print("repository owner/name unknown: set an origin remote or ACP_REPOSITORY_OWNER/NAME", file=sys.stderr)
        return 2
    try:
        status, reply = call(cfg, "GET", "/notice?" + urlencode({"owner": cfg["owner"], "name": cfg["name"]}),
                             auth=False, timeout=args.timeout)
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4
    emit(reply, cfg, sys.stdout if status == 200 else sys.stderr)
    if status != 200 or not isinstance(reply, dict):
        return 3
    if reply.get("governed") is not True:
        print("NOT GOVERNED: no active ACP policy names this repository. Register one: "
              "scripts/acp.py policy > p.json, then acp-gateway scripts/register-policy.py --from p.json",
              file=sys.stderr)
        return 0 if args.allow_ungoverned else 5
    return 0


def cmd_verify(args, cfg) -> int:
    problem = require(cfg)
    token = token_from(args)
    if problem or not token:
        print(problem or "no capability_token (ALLOW without a signing key carries none)", file=sys.stderr)
        return 2
    return post(cfg, "/internal/verify-capability", {"capability_token": token}, args)


def cmd_recovery_complete(args, cfg) -> int:
    problem = require(cfg)
    token = token_from(args)
    if problem or not token:
        print(problem or "no capability_token", file=sys.stderr)
        return 2
    body = {"capability_token": token, "result": args.result}
    if args.observed_sha:
        body["observed_sha"] = args.observed_sha
    if args.detail:
        body["detail"] = args.detail
    return post(cfg, "/internal/recovery/complete", body, args)


def cmd_board(args, cfg) -> int:
    problem = require(cfg)
    owner = args.owner or cfg["board_owner"]
    number = args.project or cfg["board_number"]
    if not problem and number and (not isinstance(number, str) or not number.isdigit()):
        problem = "board number must be a positive integer"
    if problem or not (owner and number):
        print(problem or "no board bound (.agents/board.env ACP_BOARD_OWNER/ACP_BOARD_NUMBER)", file=sys.stderr)
        return 2
    try:
        status, reply = call(cfg, "GET", "/internal/project-context?" + urlencode(
            {"owner": owner, "project": number}), timeout=max(args.timeout, 60))
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4
    if status == 200 and isinstance(reply, dict):
        advisory = reply.get("acp")
        if isinstance(advisory, dict) and isinstance(advisory.get("gate"), str) \
                and type(advisory.get("gate_blocked")) is not bool:
            # Older gateways expose only `gate`, which is the configured mode,
            # not a claim about the resource's current authorization state. A
            # successful board read proves only that the board-read gate did
            # not block this read; it does not authorize any other operation.
            advisory.setdefault("gate_mode", advisory["gate"])
            advisory["gate_status"] = "BOARD_READ_ALLOWED"
            advisory["gate_blocked"] = False
            advisory["active_block"] = None
            advisory["gate_note"] = (
                "Legacy gateway response: gate is configured mode. This successful response confirms only "
                "that this board read was allowed; it does not authorize protected actions."
            )
    emit(reply, cfg, sys.stdout if status == 200 else sys.stderr)
    return 0 if status == 200 else 3


def cmd_projects(args, cfg) -> int:
    problem = require(cfg)
    if problem:
        print(problem, file=sys.stderr)
        return 2
    path = "/internal/projects" + ("?" + urlencode({"owner": args.owner}) if args.owner else "")
    try:
        status, reply = call(cfg, "GET", path, timeout=max(args.timeout, 60))
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4
    emit(reply, cfg, sys.stdout if status == 200 else sys.stderr)
    return 0 if status == 200 else 3


def cmd_installations(args, cfg) -> int:
    problem = require(cfg)
    if problem:
        print(problem, file=sys.stderr)
        return 2
    try:
        status, reply = call(cfg, "GET", "/internal/installations", timeout=max(args.timeout, 60))
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4
    emit(reply, cfg, sys.stdout if status == 200 else sys.stderr)
    return 0 if status == 200 else 3


def cmd_policy(args, cfg) -> int:
    """The candidate an operator registers with ACP. This file grants nothing."""
    if cfg.get("identity_error"):
        print(cfg["identity_error"], file=sys.stderr)
        return 2
    if not (cfg["owner"] and cfg["name"]):
        print("repository owner/name unknown: set an origin remote or ACP_REPOSITORY_OWNER/NAME", file=sys.stderr)
        return 2
    policy = {"policy_id": args.policy_id or f"{cfg['owner']}-{cfg['name']}".lower(), "version": 1,
              "status": "active",
              "repository": {"owner": cfg["owner"], "name": cfg["name"],
                             "canonical_remote": f"https://github.com/{cfg['owner']}/{cfg['name']}.git"}}
    if cfg["board_owner"] and cfg["board_number"]:
        policy["boards"] = [{"owner": cfg["board_owner"], "number": int(cfg["board_number"])}]
    if not args.no_manifest:
        change = "admin_exact_transition" if args.strict else "authorize_and_report"
        artifacts = []
        for path, kind, required in CONTROL_ARTIFACTS:
            seen = observe(path)
            if seen["state"] != "present" or "sha256" not in seen:
                continue
            artifacts.append({"id": path.replace("/", "-").lower(), "path": path, "kind": kind,
                              "required": required, "expected_sha256": seen["sha256"], "change_policy": change})
        branch = git("symbolic-ref", "--short", "refs/remotes/origin/HEAD")
        default = branch.split("/", 1)[1] if branch and "/" in branch else "main"
        patterns = [{"id": glob.replace("/", "-").replace("*", "").strip("-").lower(), "pattern": glob,
                     "kind": kind, "change_policy": policy_for}
                    for glob, kind, policy_for in CONTROL_PATTERNS]
        policy["artifact_manifest"] = {"artifact_manifest_version": 1, "artifacts": artifacts, "patterns": patterns,
                                       "refs": [{"ref": f"refs/heads/{default}", "kind": "protected_ref",
                                                 "verification": "provider_required"}]}
    print(json.dumps(policy, indent=2, sort_keys=True))
    return 0


def cmd_endpoints(args, cfg) -> int:
    path_value = cfg.get("machine_registry_path")
    if not path_value:
        print("ACP endpoint registry path is invalid", file=sys.stderr)
        return 2
    path = Path(path_value)
    try:
        if args.endpoint_operation == "configure":
            entries = [{"name": name, "url": url, "keychainAccount": account,
                        "localOnly": args.local_only}
                       for name, url, account in args.endpoint]
            registry = acp_endpoint_registry.configure_registry(
                path, args.set_name, entries, shared_state_id=args.shared_state_id,
                replace_set=args.replace_set)
        else:
            registry = acp_endpoint_registry.read_registry(path)
    except acp_endpoint_registry.EndpointRegistryError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    output = {"registry": str(path), "formatVersion": registry["formatVersion"],
              "endpointSets": registry["endpointSets"]}
    print(json.dumps(output, indent=2, sort_keys=True))
    return 0


def cmd_override(args, cfg) -> int:
    if not (cfg.get("project_id") and cfg.get("owner") and cfg.get("name")):
        print("local override requires a project profile and canonical repository identity", file=sys.stderr)
        return 2
    try:
        if args.operation == "shared-grant":
            if cfg.get("identity_error"):
                raise ValueError(cfg["identity_error"])
            if not cfg.get("url"):
                raise ValueError("the selected ACP endpoint set is not configured")
            if args.scope == "branch":
                if not cfg.get("branch"):
                    raise ValueError("branch-scoped override requires a checked-out branch")
                ref = "refs/heads/" + cfg["branch"]
            else:
                ref = "*"
            if args.all_actions == bool(args.action):
                raise ValueError("choose exactly one of --all-actions or one or more --action values")
            if not cfg.get("remote"):
                raise ValueError("shared owner overrides require ACP_REMOTE_URL or origin")
            payload = {"resource": {"owner": cfg["owner"], "name": cfg["name"],
                                    "canonical_remote": cfg["remote"]},
                       "ref": ref, "environment": args.environment,
                       "actions": ["*"] if args.all_actions else sorted(set(args.action)),
                       "expires_in": args.expires_in, "max_uses": args.max_uses,
                       "authorized_by": args.authorized_by, "reason": args.reason}
            status, reply = call_admin(cfg, "/internal/overrides", payload, args.timeout)
            if status != 201:
                emit(reply, cfg, sys.stderr)
                return 3
            record_evidence({"gate": "acp-owner-override", "type": "shared_owner_override",
                             "disposition": "Registered", "observedAt": reply["created_at"],
                             "detail": {k: reply[k] for k in
                                        ("authorization_id", "resource", "ref", "environment", "actions",
                                         "max_uses", "expires_at", "authorized_by", "reason")}})
            emit(reply, cfg)
            return 0
        if args.operation == "shared-revoke":
            if not cfg.get("url"):
                raise ValueError("the selected ACP endpoint set is not configured")
            status, reply = call_admin(cfg, "/internal/overrides/revoke",
                                       {"authorization_id": args.override_id,
                                        "authorized_by": args.authorized_by,
                                        "reason": args.reason}, args.timeout)
            if status != 200:
                emit(reply, cfg, sys.stderr)
                return 3
            record_evidence({"gate": "acp-owner-override", "type": "shared_owner_override",
                             "disposition": "Revoked", "observedAt": reply["revoked_at"],
                             "detail": reply})
            emit(reply, cfg)
            return 0
        if args.operation == "grant":
            if args.scope == "branch" and not cfg.get("branch"):
                raise ValueError("branch-scoped override requires a checked-out branch")
            record = acp_override.grant(
                project_id=cfg["project_id"], owner=cfg["owner"], repository=cfg["name"],
                branch=cfg["branch"] if args.scope == "branch" else None,
                environment=args.environment, reason=args.reason, expires_in=args.expires_in,
                authorized_by=args.authorized_by)
            record_evidence({"gate": "acp-owner-override", "type": "local_owner_override",
                             "disposition": "Authorized", "observedAt": record["created_at"],
                             "detail": {k: record[k] for k in
                                        ("override_id", "project_id", "repository", "branch",
                                         "environment", "actions", "reason", "authorized_by", "expires_at")}})
            print(json.dumps(record, indent=2, sort_keys=True))
            return 0
        if args.operation == "revoke":
            record = acp_override.revoke(args.override_id, authorized_by=args.authorized_by,
                                         reason=args.reason or "owner revoked")
            record_evidence({"gate": "acp-owner-override", "type": "local_owner_override",
                             "disposition": "Revoked", "observedAt": record["revoked_at"],
                             "detail": record})
            print(json.dumps(record, indent=2, sort_keys=True))
            return 0
        events = acp_override._read_events(acp_override.ledger_path()) if acp_override.ledger_path() else []
        print(json.dumps(events, indent=2, sort_keys=True))
        return 0
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except Unreachable as exc:
        print(f"ACP unreachable: {exc}", file=sys.stderr)
        return 4


def apply_local_override(args, cfg, record) -> int:
    environment = getattr(args, "environment", "local")
    action = getattr(args, "action", None)
    detail = {"override_id": record["override_id"], "environment": environment,
              "branch": cfg.get("branch"), "expires_at": record["expires_at"]}
    if args.command == "authorize":
        decision = {"decision": "ALLOW", "code": "A0", "reason": "OWNER_LOCAL_OVERRIDE",
                    "action": action, "action_class": "local", "request_id": record["override_id"],
                    "evaluated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                    "authorized_action": {"action": action, "ref": args.ref,
                                          "old_sha": args.old_sha, "new_sha": args.new_sha},
                    "capability": None, "capability_token": None,
                    "policy_effect": "LOCAL_OWNER_OVERRIDE", "owner_override": detail}
        record_evidence({"gate": "acp-owner-override", "type": "local_owner_override_use",
                         "disposition": "Authorized", "observedAt": decision["evaluated_at"],
                         "workItem": args.work_item if args.record else None,
                         "detail": {**detail, "action": action, "ref": args.ref}})
        emit(decision, cfg)
        return 0
    print("ACP local check skipped under owner override " + record["override_id"] +
          " (expires " + record["expires_at"] + ")")
    record_evidence({"gate": "acp-owner-override", "type": "local_owner_override_use",
                     "disposition": "Authorized", "observedAt": datetime.now(timezone.utc).isoformat(),
                     "detail": {**detail, "command": args.command}})
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--project-root", type=Path, default=Path.cwd(),
                        help="Git worktree whose identity and ACP configuration this command uses")
    parser.add_argument("--timeout", type=float, default=15.0)
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("authorize", help="obtain a decision for one governed effect")
    p.add_argument("--action", required=True)
    p.add_argument("--environment", choices=("local", "preview", "remote", "production"))
    p.add_argument("--ref")
    p.add_argument("--old-sha")
    p.add_argument("--new-sha")
    p.add_argument("--force", action="store_true")
    p.add_argument("--recovery-authorization-id")
    p.add_argument("--owner-override-id", help="use an active shared remote owner override grant")
    p.add_argument("--work-item", help="recorded with the decision")
    p.add_argument("--context-json", help="database.migrate non-secret source_database/target_database identifiers")
    p.add_argument("--summary", help="bounded summary of why (never the prompt)")
    p.add_argument("--provenance", default="automation",
                   choices=("interactive_user", "repository_instruction", "issue_comment",
                            "pull_request_comment", "tool_output", "secondary_agent", "automation",
                            "ci", "unknown"))
    p.add_argument("--record", action="store_true", help="append to records/evidence.jsonl")
    p.set_defaults(fn=cmd_authorize)

    p = sub.add_parser("report", help="telemetry; never authorizes")
    p.add_argument("--environment", choices=("local", "preview"), default="local")
    p.add_argument("--event", required=True)
    p.add_argument("--checkpoint", choices=CHECKPOINTS)
    p.add_argument("--action")
    p.add_argument("--ref")
    p.add_argument("--detail")
    p.add_argument("--path", help="the file this event concerns; its current sha256 is attached")
    p.set_defaults(fn=cmd_report)

    p = sub.add_parser("snapshot", help="observed control-artifact state at a checkpoint")
    p.add_argument("--environment", choices=("local", "preview"), default="local")
    p.add_argument("--checkpoint", required=True, choices=CHECKPOINTS)
    p.add_argument("--path", action="append", help="default: the bootstrap's control artifacts")
    p.set_defaults(fn=cmd_snapshot)

    for name, fn in (("verify-capability", cmd_verify), ("recovery-complete", cmd_recovery_complete)):
        p = sub.add_parser(name)
        p.add_argument("--token")
        p.add_argument("--decision", help="a saved authorize output, or - for stdin")
        if name == "recovery-complete":
            p.add_argument("--result", required=True, choices=("succeeded", "failed"))
            p.add_argument("--observed-sha")
            p.add_argument("--detail")
        p.set_defaults(fn=fn)

    p = sub.add_parser("notice", help="is this repository governed (exit 5 if not)")
    p.add_argument("--environment", choices=("local", "preview"), default="local")
    p.add_argument("--allow-ungoverned", action="store_true")
    p.set_defaults(fn=cmd_notice)

    p = sub.add_parser("board", help="read the bound board through ACP")
    p.add_argument("--environment", choices=("local", "preview"), default="local")
    p.add_argument("--owner", help="override the bound board owner for this read")
    p.add_argument("--project", help="override the bound board number for this read")
    p.set_defaults(fn=cmd_board)

    p = sub.add_parser("projects", help="list boards visible through ACP")
    p.add_argument("--owner", help="limit to one GitHub account")
    p.set_defaults(fn=cmd_projects)

    p = sub.add_parser("installations", help="list GitHub accounts visible through ACP")
    p.set_defaults(fn=cmd_installations)

    p = sub.add_parser("policy", help="emit the candidate ACP policy for this repository")
    p.add_argument("--policy-id")
    p.add_argument("--strict", action="store_true",
                   help="control artifacts change only by admin transition (material on change)")
    p.add_argument("--no-manifest", action="store_true")
    p.set_defaults(fn=cmd_policy)

    p = sub.add_parser("endpoints", help="inspect or configure the machine-local ACP endpoint registry")
    endpoint_ops = p.add_subparsers(dest="endpoint_operation", required=True)
    listing = endpoint_ops.add_parser("list", help="show endpoint names and URLs, never bearer values")
    listing.set_defaults(fn=cmd_endpoints)
    configure = endpoint_ops.add_parser("configure", help="add one ordered endpoint set without storing credentials")
    configure.add_argument("--set-name", required=True)
    configure.add_argument("--endpoint", action="append", nargs=3, required=True,
                           metavar=("NAME", "HTTPS_URL", "KEYCHAIN_ACCOUNT"))
    configure.add_argument("--local-only", action="store_true",
                           help="require every endpoint in this set to be loopback")
    configure.add_argument("--shared-state-id",
                           help="stable identifier advertised by every instance sharing this transactional state")
    configure.add_argument("--replace-set", action="store_true",
                           help="replace this named set; other sets are retained")
    configure.set_defaults(fn=cmd_endpoints)

    p = sub.add_parser("override", help="manage a temporary local-only owner override")
    operations = p.add_subparsers(dest="operation", required=True)
    grant = operations.add_parser("grant", help="disable local ACP checks for a bounded local scope")
    grant.add_argument("--scope", choices=("branch", "project"), default="branch")
    grant.add_argument("--environment", choices=("local", "preview"), required=True)
    grant.add_argument("--expires-in", type=int, default=3600)
    grant.add_argument("--reason", required=True)
    grant.add_argument("--authorized-by", required=True)
    grant.set_defaults(fn=cmd_override)
    revoke = operations.add_parser("revoke", help="revoke a local override")
    revoke.add_argument("override_id")
    revoke.add_argument("--reason")
    revoke.add_argument("--authorized-by", required=True)
    revoke.set_defaults(fn=cmd_override)
    listing = operations.add_parser("list", help="list the local override ledger")
    listing.set_defaults(fn=cmd_override)
    remote = operations.add_parser("shared-grant", help="create a shared signed override for remote effects")
    remote.add_argument("--scope", choices=("branch", "project"), default="branch")
    remote.add_argument("--environment", choices=("preview", "staging", "production"), required=True)
    remote.add_argument("--action", action="append", default=[])
    remote.add_argument("--all-actions", action="store_true")
    remote.add_argument("--expires-in", type=int, default=3600)
    remote.add_argument("--max-uses", type=int, default=20)
    remote.add_argument("--reason", required=True)
    remote.add_argument("--authorized-by", required=True)
    remote.add_argument("--timeout", type=float, default=15.0)
    remote.set_defaults(fn=cmd_override)
    remote_revoke = operations.add_parser("shared-revoke", help="revoke a shared remote owner override")
    remote_revoke.add_argument("override_id")
    remote_revoke.add_argument("--reason", required=True)
    remote_revoke.add_argument("--authorized-by", required=True)
    remote_revoke.add_argument("--timeout", type=float, default=15.0)
    remote_revoke.set_defaults(fn=cmd_override)

    args = parser.parse_args()
    global PROJECT_ROOT
    requested_root = args.project_root.expanduser().resolve()
    try:
        if not requested_root.is_dir():
            raise ValueError("project root is not a directory")
        try:
            actual_root = Path(subprocess.check_output(
                ["git", "-C", str(requested_root), "rev-parse", "--show-toplevel"],
                text=True, stderr=subprocess.DEVNULL).strip()).resolve()
            is_worktree = actual_root == requested_root
        except (OSError, subprocess.CalledProcessError):
            is_worktree = False
            actual_root = requested_root
        if not is_worktree and not (
                args.command == "policy"
                and (requested_root / "PROJECT_PROFILE.json").is_file()
                and (requested_root / ".formal-bootstrap.json").is_file()):
            raise ValueError("path must be a Git worktree root (policy generation also supports a newly initialized project)")
    except (OSError, ValueError) as exc:
        print(f"ACP project root is not a readable Git worktree: {exc}", file=sys.stderr)
        return 2
    PROJECT_ROOT = requested_root
    acp_override.configure_project_root(PROJECT_ROOT)
    cfg = settings()
    if args.command == "override":
        return args.fn(args, cfg)
    environment = getattr(args, "environment", "local")
    if args.command == "authorize":
        environment = args.environment or acp_override.inferred_environment(args.action)
    record = acp_override.active(project_id=cfg.get("project_id"), owner=cfg.get("owner"),
                                 repository=cfg.get("name"), branch=cfg.get("branch"),
                                 environment=environment, action=getattr(args, "action", None))
    if record and args.command in ("authorize", "report", "snapshot", "notice", "board"):
        if args.command == "authorize" and args.action not in LOCAL_ACTIONS:
            return args.fn(args, cfg)
        return apply_local_override(args, cfg, record)
    return args.fn(args, cfg)


if __name__ == "__main__":
    raise SystemExit(main())
