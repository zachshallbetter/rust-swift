"""The everyday ACP commands: status, init, login, logout, push, record, doctor, uninstall.

They sit on top of the decision client in acp.py and never widen what it does:
every remote effect still goes through `authorize`, one exact decision per step.
"""
from __future__ import annotations

import getpass
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlencode

import acp
import acp_paths

ZERO = "0" * 40
GITIGNORE_KEEP = ("!.agents/", "!**/.agents/", "!AGENTS.md")
GITIGNORE_LOCAL = (".agents/records/local/", ".agents/.upgrades/", ".env.local")
APP_INSTALL_URL = "https://github.com/apps/acp-agent-control-plane/installations/new"


def root() -> Path:
    return acp.PROJECT_ROOT


def git(*args: str, check: bool = False) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root()), *args], capture_output=True, text=True, check=check)


def read_profile() -> tuple[Path, dict]:
    path = acp_paths.profile_path(root())
    try:
        return path, json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return path, {}


def local_log() -> Path:
    return acp_paths.local_records_dir(root()) / "acp-decisions.jsonl"


# ------------------------------------------------------------------ status

class Report:
    LEVELS = ("OK", "INFO", "WARN", "FAIL")

    def __init__(self):
        self.items: list[dict] = []

    def add(self, code: str, check: str, level: str, detail: str, fix: str | None = None) -> None:
        self.items.append({"code": code, "check": check, "level": level, "detail": detail, "fix": fix})

    def worst(self) -> str:
        return max((item["level"] for item in self.items), key=self.LEVELS.index, default="OK")

    def print(self, as_json: bool, cfg: dict) -> None:
        if as_json:
            acp.emit({"result": self.worst(), "checks": self.items}, cfg)
            return
        for item in self.items:
            line = f"{item['level']:<4} {item['code']} {item['check']:<14} {item['detail']}"
            print(acp.scrub(line, cfg))
            if item["fix"] and item["level"] in ("WARN", "FAIL"):
                print(acp.scrub(f"{'':19}fix: {item['fix']}", cfg))
        print({"OK": "ACP is ready.", "INFO": "ACP is ready.",
               "WARN": "ACP works; the warnings above have fixes.",
               "FAIL": "ACP is not ready: each FAIL above names its fix."}[self.worst()])


def file_digest(path: Path) -> str | None:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def deploy_indicators() -> list[str]:
    return [name for name in acp.DEPLOY_INDICATORS if (root() / name).exists()]


def gateway(cfg: dict, path: str, auth: bool = True, timeout: float = 10.0) -> tuple[int | None, object]:
    try:
        return acp.call(cfg, "GET", path, auth=auth, timeout=timeout)
    except acp.Unreachable as exc:
        return None, str(exc)


def cmd_status(args, cfg) -> int:
    report = Report()
    project = root()
    _, profile = read_profile()
    provider = profile.get("authorizationProvider") if isinstance(profile.get("authorizationProvider"), dict) else {}

    report.add("S001", "python", "OK", f"Python {platform.python_version()}")

    if (project / ".agents").is_symlink():
        report.add("S002", "layout", "FAIL", ".agents is a symbolic link",
                   "replace it with a real directory; ACP and the bootstrap refuse to follow it")
    elif acp_paths.contained(project):
        report.add("S002", "layout", "OK", "contained (.agents/)")
    else:
        report.add("S002", "layout", "WARN", "legacy layout (scripts/, docs/ and PROJECT_PROFILE.json at the root)",
                   "move it under .agents/: python3 scripts/upgrade-project.py --from <bootstrap 0.7 checkout> --apply")
    ignored = git("check-ignore", "-q", ".agents/acp/acp")
    if ignored.returncode == 0:
        report.add("S002", "layout", "FAIL", "git ignores .agents/, so the client would never be committed",
                   "add '!.agents/' to this repository's .gitignore (or run: .agents/acp/acp doctor --fix)")

    if not profile:
        report.add("S003", "profile", "FAIL", f"no project profile at {acp_paths.profile_path(project).relative_to(project)}",
                   ".agents/acp/acp init")
    elif provider.get("name") == "none":
        report.add("S003", "profile", "INFO", "ACP is uninstalled for this project (authorizationProvider: none)")
    elif provider.get("name") != "acp-gateway":
        report.add("S003", "profile", "FAIL", "the profile does not name acp-gateway as its authorization provider",
                   ".agents/acp/acp init")
    else:
        detail = f"fail-closed {'on' if provider.get('failClosed', True) else 'OFF'}, endpoint set {cfg.get('endpoint_set') or '-'}"
        report.add("S003", "profile", "OK", detail)

    indicators = deploy_indicators()
    if indicators and not cfg.get("auto_deploy"):
        report.add("S012", "auto-deploy", "WARN",
                   f"{', '.join(indicators)} suggests main deploys automatically, but autoDeploy is not declared",
                   ".agents/acp/acp init (declares it, so a push to main also gets production.deploy)")
    elif cfg.get("auto_deploy"):
        refs = ", ".join(f"{rule['ref']}→{rule['environment']}" for rule in cfg["auto_deploy"])
        report.add("S012", "auto-deploy", "OK", f"declared: {refs}")

    context_tool = project / ".agents/bootstrap/scripts/gen-context.py"
    if context_tool.exists():
        check = subprocess.run([sys.executable, str(context_tool), "--check"], cwd=project,
                               capture_output=True, text=True)
        if check.returncode == 0:
            report.add("S013", "context", "OK", "compiled context is current")
        else:
            reason = next((line for line in check.stderr.splitlines() if line.startswith(("STALE", "ERROR"))), "stale")
            report.add("S013", "context", "WARN", reason, ".agents/bootstrap/bootstrap context")

    version = acp_paths.client_version()
    client_digest = file_digest(acp_paths.CLIENT_HOME / "lib" / "acp.py")

    if args.offline:
        report.add("S004", "client", "INFO", f"version {version} (offline: not compared with the latest release)")
        report.print(args.json, cfg)
        return 2 if report.worst() == "FAIL" else (1 if args.strict and report.worst() == "WARN" else 0)

    if cfg.get("endpoint_problems"):
        report.add("S005", "endpoints", "FAIL", "; ".join(cfg["endpoint_problems"]),
                   f".agents/acp/acp endpoints configure --set {cfg.get('endpoint_set') or 'production'} ...")
    elif not cfg.get("endpoints"):
        report.add("S005", "endpoints", "FAIL", "no ACP endpoint has both a URL and a credential",
                   ".agents/acp/acp login")
    else:
        names = ", ".join(endpoint["name"] for endpoint in cfg["endpoints"])
        report.add("S005", "endpoints", "OK", f"set {cfg.get('endpoint_set')}: {names}")

    reachable = bool(cfg.get("endpoints")) and not cfg.get("endpoint_problems")
    if reachable:
        status, body = gateway(cfg, "/ready", auth=False)
        if status == 200 and isinstance(body, dict) and body.get("status") == "ready":
            report.add("S007", "gateway", "OK", f"ready ({body.get('storage', 'storage unknown')}) via {cfg.get('active_endpoint')}")
        elif status is None:
            report.add("S007", "gateway", "FAIL", f"unreachable: {body}",
                       "retry; a cold endpoint can take a few seconds (--timeout raises the limit)")
            reachable = False
        else:
            report.add("S007", "gateway", "FAIL", f"not ready (HTTP {status})", "check the gateway deployment")
            reachable = False

    if reachable:
        status, body = gateway(cfg, "/internal/app-status")
        source = (cfg["endpoints"][0].get("token_source") if cfg.get("endpoints") else None) or "unknown source"
        if status == 200:
            report.add("S006", "credential", "OK", f"accepted (from {source})")
        elif status == 401:
            report.add("S006", "credential", "FAIL", f"the gateway rejected the token from {source}; this is not an outage",
                       ".agents/acp/acp login (stores a current token in the Keychain)")
            reachable = False
        else:
            report.add("S006", "credential", "WARN", f"could not verify (HTTP {status})")

    if reachable:
        status, body = gateway(cfg, "/internal/client-status?" + urlencode(
            {"version": version, "sha256": client_digest or ""}))
        if status == 200 and isinstance(body, dict):
            latest, known, current = body.get("latest"), body.get("known"), body.get("current")
            if current:
                report.add("S004", "client", "OK", f"{version} (current release)")
            elif known:
                report.add("S004", "client", "WARN", f"{version}; bootstrap {latest} is available",
                           ".agents/bootstrap/bootstrap upgrade")
            else:
                report.add("S004", "client", "WARN",
                           f"{version}, but this client's files match no released version (edited locally?)",
                           ".agents/bootstrap/bootstrap repair (restores the released files)")
        else:
            report.add("S004", "client", "INFO", f"version {version} (this gateway does not report releases yet)")

    if cfg.get("identity_error"):
        report.add("S008", "repository", "FAIL", cfg["identity_error"], "fix the origin remote or ACP_REPOSITORY_OWNER/NAME")
    elif not (cfg.get("owner") and cfg.get("name")):
        report.add("S008", "repository", "FAIL", "repository owner/name unknown (no origin remote)",
                   "git remote add origin https://github.com/OWNER/NAME.git, then .agents/acp/acp init")
    elif reachable:
        status, body = gateway(cfg, "/notice?" + urlencode({"owner": cfg["owner"], "name": cfg["name"]}), auth=False)
        if status == 200 and isinstance(body, dict):
            if body.get("governed") is not True:
                level = "FAIL" if cfg.get("fail_closed") else "WARN"
                report.add("S008", "policy", level, f"no ACP policy governs {cfg['owner']}/{cfg['name']}; "
                           "protected actions are refused",
                           f"the repository owner runs (acp-gateway) scripts/policy-admin.py register "
                           f"{cfg['owner']}/{cfg['name']} --apply")
            else:
                state = body.get("authorization_state")
                if state not in (None, "NORMAL"):
                    report.add("S008", "policy", "FAIL", f"{body.get('policy_id')}: {state} "
                               f"(incident {body.get('incident_id')})",
                               "a security administrator reviews the incident and runs the recovery flow")
                else:
                    report.add("S008", "policy", "OK", f"{body.get('policy_id')} v{body.get('policy_version')}, NORMAL")
                integrity = body.get("integrity") if isinstance(body.get("integrity"), dict) else {}
                if integrity.get("unresolved"):
                    report.add("S010", "integrity", "WARN", f"pinned control files differ: {integrity.get('status')}",
                               "the repository owner approves the change: (acp-gateway) scripts/policy-admin.py "
                               f"grant {cfg['owner']}/{cfg['name']} --checkout . --apply")
        else:
            report.add("S008", "policy", "WARN", f"could not read the repository notice (HTTP {status})")

        status, body = gateway(cfg, "/internal/installations")
        if status == 200 and isinstance(body, dict):
            accounts = {str(item.get("account") or item.get("login") or "").lower()
                        for item in body.get("installations") or body.get("accounts") or []}
            if cfg["owner"].lower() in accounts:
                report.add("S009", "github-app", "OK", f"installed for {cfg['owner']}")
            else:
                report.add("S009", "github-app", "FAIL",
                           f"the ACP GitHub App is not installed for {cfg['owner']}, so pushes cannot be verified",
                           f"install it for {cfg['owner']} (only this repository is needed): {APP_INSTALL_URL}")

    board_owner, board_number = cfg.get("board_owner"), cfg.get("board_number")
    if not (board_owner and board_number):
        report.add("S011", "board", "INFO", "no board bound", ".agents/acp/acp init --board OWNER/NUMBER")
    elif reachable:
        status, body = gateway(cfg, "/internal/projects?" + urlencode({"owner": board_owner}))
        boards = (body.get("projects") or body.get("boards") or []) if isinstance(body, dict) else []
        match = next((b for b in boards if str(b.get("number")) == str(board_number)), None)
        if match and match.get("allowed") is not False:
            report.add("S011", "board", "OK", f"{board_owner}#{board_number} {match.get('title') or ''}".strip())
        else:
            report.add("S011", "board", "WARN", f"{board_owner}#{board_number} is not visible or not allowed",
                       "check the App installation and the board allowlist")

    report.print(args.json, cfg)
    worst = report.worst()
    return 2 if worst == "FAIL" else (1 if args.strict and worst == "WARN" else 0)


# ------------------------------------------------------------------ init

def write_text(path: Path, text: str, dry_run: bool, label: str) -> None:
    if path.exists() and path.read_text(encoding="utf-8") == text:
        return
    print(f"{'would write' if dry_run else 'wrote'} {label}")
    if not dry_run:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def cmd_init(args, cfg) -> int:
    project = root()
    profile_path, profile = read_profile()
    if not profile_path.exists():
        profile_path = project / acp_paths.CONTAINED_PROFILE
    provider = dict(profile.get("authorizationProvider") or {})
    missing: list[str] = []

    owner, name = cfg.get("owner"), cfg.get("name")
    upstream = git("remote", "get-url", "upstream").stdout.strip()
    if upstream:
        up_owner, up_name = acp.owner_and_name(upstream)
        if (up_owner, up_name) != (owner, name):
            print(f"note: origin is {owner}/{name} and upstream is {up_owner}/{up_name}; ACP governs origin. "
                  "Set ACP_REMOTE_URL in .agents/board.env if the upstream is the governed repository.")
    if not (owner and name):
        missing.append("a GitHub origin remote (git remote add origin https://github.com/OWNER/NAME.git)")

    provider.setdefault("name", "acp-gateway")
    provider.setdefault("status", "declared")
    provider.setdefault("failClosed", True)
    provider.setdefault("authorizedEffectClassesMinimum", "E2")
    provider.setdefault("boardEnvFile", ".agents/board.env")
    provider["gatewayEndpointSet"] = args.endpoint_set or provider.get("gatewayEndpointSet") or "production"
    for key in ("gatewayUrlEnv", "gatewayTokenEnv", "checkScript"):
        provider.pop(key, None)
    provider.setdefault("clientScript", ".agents/acp/acp")

    board = dict(provider.get("board") or {})
    if args.board:
        board_owner, _, number = args.board.partition("/")
        if not number.isdigit():
            print("--board takes OWNER/NUMBER", file=sys.stderr)
            return 2
        board = {"owner": board_owner, "number": int(number)}
    elif not (board.get("owner") and board.get("number")) and owner and cfg.get("endpoints") and not args.offline:
        status, body = gateway(cfg, "/internal/projects?" + urlencode({"owner": owner}))
        boards = [b for b in ((body.get("projects") or body.get("boards") or []) if isinstance(body, dict) else [])
                  if b.get("allowed") is not False]
        if len(boards) == 1:
            board = {"owner": owner, "number": int(boards[0]["number"])}
            print(f"board: bound {owner}#{board['number']} ({boards[0].get('title')}), the only one ACP can see")
        elif boards:
            print(f"board: {len(boards)} boards are visible for {owner}; choose one with --board {owner}/NUMBER:")
            for item in boards[:20]:
                print(f"  {owner}/{item.get('number')}  {item.get('title')}")
    provider["board"] = {"owner": board.get("owner"), "number": board.get("number")}

    indicators = deploy_indicators()
    if args.auto_deploy is not None:
        provider["autoDeploy"] = [{"ref": "refs/heads/main", "environment": "production"}] if args.auto_deploy else []
    elif indicators and not provider.get("autoDeploy"):
        provider["autoDeploy"] = [{"ref": "refs/heads/main", "environment": "production"}]
        print(f"auto-deploy: {', '.join(indicators)} found, so refs/heads/main is declared as deploying production")
    provider.setdefault("autoDeploy", [])

    endpoint_ok = not cfg.get("endpoint_problems") and bool(cfg.get("endpoints"))
    if not endpoint_ok:
        missing.append(f"the machine endpoint set {provider['gatewayEndpointSet']!r} with a credential "
                       "(.agents/acp/acp endpoints configure …, then .agents/acp/acp login)")
    provider["status"] = "bound" if owner and name and board.get("owner") and endpoint_ok else "declared"
    profile["authorizationProvider"] = provider
    profile.setdefault("projectId", (name or project.name).lower())
    profile.setdefault("projectName", name or project.name)
    write_text(profile_path, json.dumps(profile, indent=2) + "\n", args.dry_run, str(profile_path.relative_to(project)))

    lines = ["# ACP repository identity and board binding. Committed; never a credential."]
    if owner and name:
        lines.append(f"ACP_REMOTE_URL=https://github.com/{owner}/{name}.git")
    if board.get("owner") and board.get("number"):
        lines += [f"ACP_BOARD_OWNER={board['owner']}", f"ACP_BOARD_NUMBER={board['number']}",
                  f"GH_PROJECT_OWNER={board['owner']}", f"GH_PROJECT_NUMBER={board['number']}"]
    write_text(project / ".agents/board.env", "\n".join(lines) + "\n", args.dry_run, ".agents/board.env")

    gitignore = project / ".gitignore"
    existing = gitignore.read_text(encoding="utf-8").splitlines() if gitignore.exists() else []
    additions = [line for line in (*GITIGNORE_LOCAL, *GITIGNORE_KEEP) if line not in existing]
    if additions:
        block = ["", "# ACP: per-machine state stays out; .agents/ and AGENTS.md stay in", *additions]
        write_text(gitignore, "\n".join(existing + block) + "\n", args.dry_run, ".gitignore")

    if missing:
        print("still needed:")
        for item in missing:
            print(f"  - {item}")
    print("next: .agents/acp/acp status")
    return 0


# ------------------------------------------------------------------ login / logout

def keychain_account(cfg: dict) -> str:
    return cfg.get("token_env") or "ACP_GATEWAY_TOKEN"


def cmd_login(args, cfg) -> int:
    account = args.account or keychain_account(cfg)
    if platform.system() != "Darwin" or not shutil.which("security"):
        print(f"No macOS Keychain here. Set {account} in the environment (CI secrets, or a git-ignored .env.local) "
              "instead; ACP reads it from there.")
        return 0
    print(f"Storing the ACP token in the Keychain item '{acp.KEYCHAIN_SERVICE}' / {account}.")
    print("Paste it when asked (it is not shown, and not passed on any command line).")
    result = subprocess.run(["security", "add-generic-password", "-U", "-s", acp.KEYCHAIN_SERVICE, "-a", account,
                             "-w"], check=False)
    if result.returncode != 0:
        print("the Keychain did not store the token", file=sys.stderr)
        return 2
    fresh = acp.settings()
    status, _ = gateway(fresh, "/internal/app-status") if fresh.get("endpoints") else (None, None)
    print("token stored and accepted by the gateway" if status == 200 else
          "token stored; the gateway did not accept it yet (run .agents/acp/acp status)")
    return 0 if status == 200 else 1


def cmd_logout(args, cfg) -> int:
    account = args.account or keychain_account(cfg)
    if platform.system() != "Darwin" or not shutil.which("security"):
        print(f"No Keychain here; unset {account} wherever you set it.")
        return 0
    if not args.yes:
        answer = input(f"Delete the Keychain item '{acp.KEYCHAIN_SERVICE}' / {account}? [y/N] ")
        if answer.strip().lower() not in ("y", "yes"):
            print("kept")
            return 0
    result = subprocess.run(["security", "delete-generic-password", "-s", acp.KEYCHAIN_SERVICE, "-a", account],
                            capture_output=True, text=True)
    print("deleted" if result.returncode == 0 else "no such Keychain item")
    return 0


# ------------------------------------------------------------------ push

def run_acp(*argv: str) -> tuple[int, dict, str]:
    command = [sys.executable, str(Path(acp.__file__).resolve()), "--project-root", str(root()), *argv]
    result = subprocess.run(command, capture_output=True, text=True)
    try:
        body = json.loads(result.stdout) if result.stdout.strip().startswith("{") else {}
    except ValueError:
        body = {}
    return result.returncode, body, result.stderr


def remote_sha(ref: str) -> str | None:
    out = git("ls-remote", "origin", ref).stdout.split()
    return out[0] if out else None


def run_pre_push(remote_ref: str, sha: str, old: str) -> bool:
    hook_dir = git("rev-parse", "--git-path", "hooks").stdout.strip()
    hook = (root() / hook_dir if not Path(hook_dir).is_absolute() else Path(hook_dir)) / "pre-push"
    if not hook.is_file() or not os.access(hook, os.X_OK):
        return True
    print(f"running the project's pre-push hook for {remote_ref} before asking ACP …")
    stdin = f"refs/heads/acp-push {sha} {remote_ref} {old}\n"
    result = subprocess.run(["git", "-C", str(root()), "hook", "run", "pre-push", "--to-stdin=/dev/stdin", "--",
                             "origin", git("remote", "get-url", "origin").stdout.strip()],
                            input=stdin, text=True)
    return result.returncode == 0


def authorize_and_push(action: str, ref: str, old: str, new: str, summary: str) -> bool:
    for step in (["report", "--event", "PROTECTED_EFFECT_PENDING", "--checkpoint", "PRE_PUSH", "--action", action,
                  "--ref", ref], ["snapshot", "--checkpoint", "PRE_PUSH"]):
        code, _, err = run_acp(*step)
        if code not in (0,):
            print(f"{step[0]} failed: {err.strip()[:300]}", file=sys.stderr)
            return False
    code, decision, err = run_acp("authorize", "--action", action, "--ref", ref, "--old-sha", old, "--new-sha", new,
                                  "--summary", summary, "--provenance", "interactive_user")
    deploy = decision.get("deploy_decision")
    if deploy:
        print(f"  production.deploy: {deploy.get('decision')} {deploy.get('reason')} {deploy.get('request_id')}")
    print(f"  {action} {ref}: {decision.get('decision')} {decision.get('reason')} {decision.get('request_id')}")
    if code != 0:
        for line in err.strip().splitlines()[-3:]:
            print(f"  {line}", file=sys.stderr)
        remedy = (decision.get("gateway_decision") or {}).get("remedy") or decision.get("remedy")
        if remedy:
            print(f"  remedy: {remedy}", file=sys.stderr)
        return False
    push = subprocess.run(["git", "-C", str(root()), "push", "--no-verify", "-q", "origin", f"{new}:{ref}"],
                          capture_output=True, text=True)
    if push.returncode != 0 or remote_sha(ref) != new:
        print(f"  push to {ref} failed: {push.stderr.strip()[:300]}", file=sys.stderr)
        return False
    print(f"  pushed {new[:7]} to {ref}")
    return True


def cmd_push(args, cfg) -> int:
    sha = git("rev-parse", args.rev).stdout.strip()
    if not sha:
        print(f"no such revision: {args.rev}", file=sys.stderr)
        return 2
    branch = git("symbolic-ref", "--quiet", "--short", "HEAD").stdout.strip()
    target = args.to or branch
    if not target:
        print("detached HEAD: name the branch to update with --to", file=sys.stderr)
        return 2
    target_ref = f"refs/heads/{target}"
    default = (git("symbolic-ref", "--quiet", "refs/remotes/origin/HEAD").stdout.strip().rsplit("/", 1)[-1]
               or "main")
    protected = target in (default, "main", "master") or bool(args.protected)
    git("fetch", "-q", "origin")
    old = remote_sha(target_ref) or ZERO
    if old != ZERO and git("merge-base", "--is-ancestor", old, sha).returncode != 0:
        print(f"{target} on origin is not an ancestor of {sha[:7]}; ACP push only fast-forwards "
              "(rebase or merge first)", file=sys.stderr)
        return 2
    if old == sha:
        print(f"{target} is already at {sha[:7]}")
        return 0
    if args.dry_run:
        steps = ([f"publish {sha[:7]} to refs/heads/{args.stage or 'acp/' + sha[:12]} (git.remote.update)"]
                 if protected else [])
        steps.append(f"update {target_ref} {old[:7]} → {sha[:7]} "
                     f"({'git.protected_ref.update' if protected else 'git.remote.update'})")
        if protected and any(rule["ref"] == target_ref for rule in cfg.get("auto_deploy") or []):
            steps.append("production.deploy, obtained with the ref update (autoDeploy)")
        print("would:\n  " + "\n  ".join(steps))
        return 0
    if not args.no_verify and not run_pre_push(target_ref, sha, old):
        print("the pre-push hook refused; nothing was asked of ACP or pushed", file=sys.stderr)
        return 3
    summary = args.summary or f"acp push {sha[:7]} to {target}"
    if protected:
        stage = args.stage or f"acp/{sha[:12]}"
        stage_ref = f"refs/heads/{stage}"
        if not authorize_and_push("git.remote.update", stage_ref, remote_sha(stage_ref) or ZERO, sha, summary):
            return 4
        if not authorize_and_push("git.protected_ref.update", target_ref, old, sha, summary):
            return 4
        if not args.stage:
            code, decision, _ = run_acp("authorize", "--action", "git.remote.update", "--ref", stage_ref,
                                        "--old-sha", sha, "--new-sha", ZERO, "--summary", f"remove {stage} after push")
            if code == 0:
                git("push", "--no-verify", "-q", "origin", f":{stage_ref}")
            else:
                print(f"  kept {stage} on origin (ACP did not allow removing it: {decision.get('reason')})")
    elif not authorize_and_push("git.remote.update", target_ref, old, sha, summary):
        return 4
    return 0


# ------------------------------------------------------------------ record

def cmd_record(args, cfg) -> int:
    path = local_log()
    entries = []
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            try:
                entries.append(json.loads(line))
            except ValueError:
                continue
    if args.list or not (args.request_id or args.all):
        for entry in entries[-args.limit:]:
            print(f"{entry.get('observedAt', '')[:19]}  {entry.get('request_id') or '-':<16}  "
                  f"{entry.get('decision') or '-':<6} {entry.get('action') or '-':<26} "
                  f"{(entry.get('context') or {}).get('ref') or ''}")
        if not entries:
            print("no decisions logged on this machine yet")
        return 0
    chosen = entries if args.all else [e for e in entries if e.get("request_id") in set(args.request_id)]
    if not chosen:
        print("no matching decisions", file=sys.stderr)
        return 2
    ledger = acp_paths.records_dir(root()) / "evidence.jsonl"
    ledger.parent.mkdir(parents=True, exist_ok=True)
    known = ledger.read_text(encoding="utf-8") if ledger.exists() else ""
    added = 0
    with ledger.open("a", encoding="utf-8") as handle:
        for entry in chosen:
            if entry.get("request_id") and entry["request_id"] in known:
                continue
            handle.write(json.dumps({"gate": "acp-decision", "type": "acp_authorization",
                                     "disposition": "Observed", **entry}, sort_keys=True) + "\n")
            added += 1
    print(f"added {added} decision(s) to {ledger.relative_to(root())}; commit it with your next change")
    return 0


# ------------------------------------------------------------------ doctor

def cmd_doctor(args, cfg) -> int:
    project = root()
    fixes = []
    gitignore = project / ".gitignore"
    existing = gitignore.read_text(encoding="utf-8").splitlines() if gitignore.exists() else []
    if git("check-ignore", "-q", ".agents/acp/acp").returncode == 0 or any(
            line not in existing for line in GITIGNORE_LOCAL):
        fixes.append(("keep .agents/ tracked and per-machine state ignored in .gitignore", lambda: append_gitignore(gitignore, existing)))
    tool = project / ".agents/bootstrap/scripts/gen-context.py"
    if tool.exists() and subprocess.run([sys.executable, str(tool), "--check"], cwd=project,
                                        capture_output=True).returncode != 0:
        fixes.append(("regenerate the compiled context", lambda: subprocess.run([sys.executable, str(tool)], cwd=project)))
    if tool.exists():
        hooks = git("rev-parse", "--git-path", "hooks").stdout.strip()
        hook = (project / hooks if hooks and not Path(hooks).is_absolute() else Path(hooks or ".git/hooks")) / "pre-commit"
        if not hook.exists():
            fixes.append(("install the pre-commit hook that keeps the context lock current",
                          lambda: subprocess.run([sys.executable, str(tool), "--install-hook"], cwd=project)))
    if deploy_indicators() and not cfg.get("auto_deploy"):
        fixes.append(("declare that main deploys production (autoDeploy)", lambda: declare_auto_deploy()))
    if not fixes:
        print("nothing to fix locally; run .agents/acp/acp status for the full picture")
        return 0
    for label, apply in fixes:
        if not args.fix:
            print(f"would: {label}")
            continue
        if not args.yes and input(f"{label}? [Y/n] ").strip().lower() in ("n", "no"):
            continue
        apply()
        print(f"done: {label}")
    if not args.fix:
        print("pass --fix to apply (each is asked; --yes applies all)")
    return 0


def append_gitignore(path: Path, existing: list[str]) -> None:
    additions = [line for line in (*GITIGNORE_LOCAL, *GITIGNORE_KEEP) if line not in existing]
    if additions:
        path.write_text("\n".join(existing + ["", "# ACP: per-machine state stays out; .agents/ stays in",
                                              *additions]) + "\n", encoding="utf-8")


def declare_auto_deploy() -> None:
    path, profile = read_profile()
    provider = profile.setdefault("authorizationProvider", {})
    provider["autoDeploy"] = [{"ref": "refs/heads/main", "environment": "production"}]
    path.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")


# ------------------------------------------------------------------ uninstall

def cmd_uninstall(args, cfg) -> int:
    project = root()
    client = project / ".agents/acp"
    path, profile = read_profile()
    governed = None
    if cfg.get("owner") and cfg.get("endpoints") and not cfg.get("endpoint_problems"):
        status, body = gateway(cfg, "/notice?" + urlencode({"owner": cfg["owner"], "name": cfg["name"]}), auth=False)
        governed = isinstance(body, dict) and body.get("governed") is True
    plan = [f"move {client.relative_to(project)}/ to .agents/.upgrades/acp-removed-<time>/ (a backup, git-ignored)",
            f"set {path.relative_to(project)} authorizationProvider to {{\"name\": \"none\"}}",
            "keep .agents/records/ and every decision already recorded"]
    print("ACP uninstall " + ("plan:" if not args.apply else "steps:"))
    for line in plan:
        print(f"  - {line}")
    if governed:
        print(f"warning: a policy still governs {cfg['owner']}/{cfg['name']} on the gateway, so ACP keeps refusing "
              "protected actions there. The repository owner removes it with (acp-gateway) "
              f"scripts/policy-admin.py unregister {cfg['owner']}/{cfg['name']} --apply")
    if not args.apply:
        print("dry run: pass --apply to uninstall")
        return 0
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    backup = project / ".agents/.upgrades" / f"acp-removed-{stamp}"
    backup.parent.mkdir(parents=True, exist_ok=True)
    if profile:
        profile["authorizationProvider"] = {"name": "none", "removedAt": datetime.now(timezone.utc).isoformat()}
        path.write_text(json.dumps(profile, indent=2) + "\n", encoding="utf-8")
    if client.exists():
        shutil.move(str(client), str(backup))
    print(f"uninstalled; the client is kept at {backup.relative_to(project)} until you delete it. "
          ".agents/acp/acp logout is no longer available, so remove the Keychain item with "
          f"security delete-generic-password -s '{acp.KEYCHAIN_SERVICE}' -a {keychain_account(cfg)}")
    return 0


# ------------------------------------------------------------------ parser

def register(sub) -> None:
    p = sub.add_parser("status", help="is ACP set up and current? one line per check, each with its fix")
    p.add_argument("--json", action="store_true")
    p.add_argument("--offline", action="store_true", help="local checks only")
    p.add_argument("--strict", action="store_true", help="exit 1 on warnings")
    p.set_defaults(fn=cmd_status)

    p = sub.add_parser("init", help="generate or refresh this project's ACP settings (profile, board.env, .gitignore)")
    p.add_argument("--board", help="OWNER/NUMBER of the ProjectsV2 board to bind")
    p.add_argument("--endpoint-set", help="machine endpoint set (default: the profile's, else production)")
    p.add_argument("--auto-deploy", dest="auto_deploy", action="store_true", default=None,
                   help="declare that main deploys production")
    p.add_argument("--no-auto-deploy", dest="auto_deploy", action="store_false")
    p.add_argument("--offline", action="store_true", help="do not ask the gateway for boards")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--yes", action="store_true", help="non-interactive (CI)")
    p.set_defaults(fn=cmd_init)

    p = sub.add_parser("login", help="store the gateway token in the macOS Keychain (hidden prompt)")
    p.add_argument("--account", help="Keychain account (default: the endpoint's)")
    p.set_defaults(fn=cmd_login)

    p = sub.add_parser("logout", help="remove the gateway token from the Keychain (asks first)")
    p.add_argument("--account")
    p.add_argument("--yes", action="store_true")
    p.set_defaults(fn=cmd_logout)

    p = sub.add_parser("push", help="publish a commit through ACP: hook, then one exact approval per step")
    p.add_argument("rev", nargs="?", default="HEAD")
    p.add_argument("--to", help="branch to update (default: the current branch)")
    p.add_argument("--stage", help="branch to publish the commit on first (default: a temporary acp/<sha>)")
    p.add_argument("--protected", action="store_true", help="treat the target as a protected ref")
    p.add_argument("--summary", help="why, recorded with the decisions")
    p.add_argument("--no-verify", action="store_true", help="skip the pre-push hook (it is otherwise run first)")
    p.add_argument("--dry-run", action="store_true")
    p.set_defaults(fn=cmd_push)

    p = sub.add_parser("record", help="list this machine's ACP decisions, or copy chosen ones into the evidence ledger")
    p.add_argument("request_id", nargs="*")
    p.add_argument("--all", action="store_true")
    p.add_argument("--list", action="store_true")
    p.add_argument("--limit", type=int, default=20)
    p.set_defaults(fn=cmd_record)

    p = sub.add_parser("doctor", help="find safe local fixes; --fix applies them")
    p.add_argument("--fix", action="store_true")
    p.add_argument("--yes", action="store_true")
    p.set_defaults(fn=cmd_doctor)

    p = sub.add_parser("uninstall", help="remove the ACP client from this project (dry run unless --apply)")
    p.add_argument("--apply", action="store_true")
    p.set_defaults(fn=cmd_uninstall)
