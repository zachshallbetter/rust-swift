#!/usr/bin/env python3
"""Validate either the bootstrap repository or an initialized project."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from layout import (  # noqa: E402
    ACP_LIB_FILES, ACP_SCHEMA_FILES, RECORD_FILES, SHIPPED_DOCS, TEMPLATES, Layout, find_project_root,
)

HERE = Path(__file__).resolve().parent
ROOT = find_project_root(HERE)
LAYOUT = Layout(ROOT)
PROJECT_MODE = LAYOUT.path("installation").exists()
SHIPPED_SCRIPTS = ("gen-context.py", "upgrade-project.py", "reconcile-project.py", "validate-local-work-items.py",
                   "validate-protocol-adapters.py", "validate-qualification.py", "validate-interaction-contract.py",
                   "recipient-status.py", "check-protocol-integrity.py", "esp-instance.py", "esp-record.py",
                   "iepe-adopt.py", "validate-semantic-surface.py", "layout.py", "bootstrap_cli.py")
BOOTSTRAP_SCHEMAS = ("agent-message.schema.json", "context-lock.schema.json", "context-sources.schema.json",
                     "formal-resource-manifest.schema.json", "work-disposition.schema.json",
                     "protocol-adapter.schema.json", "qualification-record.schema.json",
                     "recipient-contract.schema.json", "interaction-contract.schema.json",
                     "external-state-instance.schema.json", "semantic-surface.schema.json")
RULES = ("standards.md", "workflow.md", "codestyle.md", "types.md", "ci.md", "governance.md", "security.md")
COMMON = [
    LAYOUT.rel("version"), "AGENTS.md", ".agents/bootstrap/bootstrap", ".agents/bootstrap/RELEASE.json",
    ".agents/acp/VERSION", ".agents/acp/acp", ".agents/acp/README.md",
    *(f".agents/bootstrap/docs/{name}" for name in SHIPPED_DOCS if name != "ACP_INTEGRATION.md"),
    *(f".agents/bootstrap/schemas/{name}" for name in BOOTSTRAP_SCHEMAS),
    *(f".agents/acp/schemas/{name}" for name in ACP_SCHEMA_FILES),
    *(f".agents/acp/lib/{name}" for name in (*ACP_LIB_FILES, "acp_paths.py")),
    *(f".agents/bootstrap/scripts/{name}" for name in SHIPPED_SCRIPTS),
    ".agents/skills/authorize-protected-effect/SKILL.md",
    ".agents/skills/github-projects/SKILL.md",
]
ACP_DECISIONS = {"ALLOW", "DENY", "AUTH_REQUIRED", "REVERIFY_REQUIRED", "QUARANTINE",
                 "LOCKED", "RECOVERY_AUTHORIZED", "VERIFY_RECOVERY"}
REPOSITORY_ONLY = [
    "README.md", "AUTHORITY.md", "VERSIONING.md", "GOVERNANCE.md",
    *(f"{TEMPLATES}/{name}" for name in ("intent.md", "profile.json", "formal-resources.json",
                                         "context-sources.json", "bindings.yaml",
                                         "contracts/acp-protected-effects.yaml")),
    *(f"{TEMPLATES}/rules/{name}" for name in RULES),
    ".agents/bootstrap/scripts/init-project.py",
    ".agents/bootstrap/scripts/package-release.py",
]
PROJECT_ONLY = [
    "README.md",
    *(LAYOUT.rel(name) for name in ("installation", "intent", "profile", "formal_resources", "context_sources",
                                    "bindings")),
    f"{LAYOUT.rel('contracts')}/acp-protected-effects.yaml",
    *(f"{LAYOUT.rel('rules')}/{name}" for name in RULES),
    *(f"{LAYOUT.rel('records')}/{name}" for name in RECORD_FILES),
]


def project_file(ref: str) -> Path:
    """A profile reference; in the bootstrap repository a project path names its template."""
    if not PROJECT_MODE and ref.startswith(".agents/project/"):
        return ROOT / TEMPLATES / ref[len(".agents/project/"):]
    return ROOT / ref


def readme_findings(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    headings: list[tuple[int, str]] = []
    outside_fence: list[str] = []
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith(("```", "~~~")):
            in_fence = not in_fence
            continue
        if not in_fence:
            outside_fence.append(line)
            match = re.match(r"^(#{1,6})\s+(.+?)\s*#*\s*$", line)
            if match:
                headings.append((len(match.group(1)), match.group(2).strip()))

    findings: list[str] = []
    nonempty = [line.strip() for line in outside_fence if line.strip()]
    if not nonempty or not re.match(r"^#\s+\S", nonempty[0]):
        findings.append("start with one descriptive H1 title")
    if sum(level == 1 for level, _ in headings) != 1:
        findings.append("use exactly one H1 title")
    if len(nonempty) < 2 or nonempty[1].startswith("#") or len(nonempty[1]) < 30:
        findings.append("include a concise factual summary after the title")

    labels = {re.sub(r"[^a-z0-9]+", " ", title.lower()).strip() for _, title in headings}
    groups = {
        "quick start": {"quick start", "getting started", "quickstart", "usage", "install", "installation"},
        "repository map": {"repository structure", "project structure", "repository map", "project map",
                           "repository layout", "project layout", "contents"},
        "verification": {"verification", "validation", "tests", "testing", "development"},
        "status and limits": {"status and limitations", "status and limits", "status honesty", "current status", "maturity"},
    }
    for label, alternatives in groups.items():
        if not labels.intersection(alternatives):
            findings.append(f"include a {label} section")
    return findings


def acp_removed() -> bool:
    """`.agents/acp/acp uninstall` leaves authorizationProvider {"name": "none"}."""
    try:
        profile = json.loads(project_file(LAYOUT.rel("profile")).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return False
    return (profile.get("authorizationProvider") or {}).get("name") == "none"


def validate_acp(profile: dict) -> int:
    acp = profile.get("authorizationProvider")
    if isinstance(acp, dict) and acp.get("name") == "none":
        return 0
    if not isinstance(acp, dict):
        print("INVALID: profile must declare authorizationProvider (see .agents/acp/README.md)", file=sys.stderr)
        return 2
    if acp.get("name") != "acp-gateway":
        print("INVALID: authorizationProvider.name must be acp-gateway", file=sys.stderr)
        return 2
    if acp.get("status") not in ("declared", "bound", "declared|bound"):
        print("INVALID: authorizationProvider.status must be declared or bound", file=sys.stderr)
        return 2
    if acp.get("failClosed") is not True:
        print("INVALID: authorizationProvider.failClosed must be true", file=sys.stderr)
        return 2
    endpoint_set = acp.get("gatewayEndpointSet")
    legacy_endpoints = acp.get("gatewayEndpoints")
    if endpoint_set is not None and (not isinstance(endpoint_set, str)
                                     or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", endpoint_set)):
        print("INVALID: authorizationProvider.gatewayEndpointSet must be a lowercase named set", file=sys.stderr)
        return 2
    if endpoint_set is not None and legacy_endpoints is not None:
        print("INVALID: profile cannot define both gatewayEndpointSet and legacy gatewayEndpoints", file=sys.stderr)
        return 2
    if legacy_endpoints is not None and (not isinstance(legacy_endpoints, list) or len(legacy_endpoints) != 1
                                         or not isinstance(legacy_endpoints[0], dict)):
        print("INVALID: only a legacy single-endpoint ACP profile can be normalized", file=sys.stderr)
        return 2
    if acp.get("authorizedEffectClassesMinimum") not in ("E0", "E1", "E2", "E3"):
        print("INVALID: authorizedEffectClassesMinimum must be E0..E3 (E4 is always governed)", file=sys.stderr)
        return 2
    for key in ("integrationRef", "decisionSchema", "actionMap"):
        ref = acp.get(key)
        if not ref or not project_file(ref).exists():
            print(f"INVALID: authorizationProvider.{key} must reference an existing file ({ref})", file=sys.stderr)
            return 2
    # Optional until every adopting project has re-pinned to the current release.
    client = acp.get("clientScript")
    if client is not None and not project_file(client).exists():
        print(f"INVALID: authorizationProvider.clientScript must reference an existing file ({client})", file=sys.stderr)
        return 2
    schema = json.loads(project_file(acp["decisionSchema"]).read_text(encoding="utf-8"))
    decisions = set(schema["properties"]["decision"]["enum"])
    if decisions != ACP_DECISIONS:
        print("INVALID: acp-decision schema decision vocabulary drifted from A0-A7", file=sys.stderr)
        return 2
    action_map = project_file(acp["actionMap"]).read_text(encoding="utf-8")
    try:
        import yaml  # optional at runtime; the substring check below still runs without it
    except ImportError:
        yaml = None
    if yaml is not None:
        try:
            parsed = yaml.safe_load(action_map)
        except yaml.YAMLError as exc:
            print(f"INVALID: {acp['actionMap']} is not valid YAML: {exc}".splitlines()[0], file=sys.stderr)
            return 2
        missing = ACP_DECISIONS - set((parsed or {}).get("decisions") or {})
        if missing:
            print(f"INVALID: {acp['actionMap']} lacks a disposition for {', '.join(sorted(missing))}", file=sys.stderr)
            return 2
    for decision in ACP_DECISIONS:
        if decision + ":" not in action_map:
            print(f"INVALID: {acp['actionMap']} lacks a disposition for {decision}", file=sys.stderr)
            return 2
    if profile.get("executionProvider", {}).get("protection") != "acp-gateway":
        print("INVALID: executionProvider.protection must be acp-gateway", file=sys.stderr)
        return 2
    if acp.get("status") == "bound":
        board_env = ROOT / acp.get("boardEnvFile", ".agents/board.env")
        contents = board_env.read_text(encoding="utf-8") if board_env.exists() else ""
        has_endpoint_binding = endpoint_set is not None or bool(acp.get("gatewayUrlEnv"))
        if not has_endpoint_binding or not board_env.exists() or "ACP_REMOTE_URL=" not in contents:
            print("INVALID: bound authorizationProvider requires an endpoint binding and repository identity in "
                  + str(board_env.relative_to(ROOT)), file=sys.stderr)
            return 2
        if any(line.split("=", 1)[0].strip().startswith(("ACP_GATEWAY_TOKEN", "ACP_POLICY_ADMIN_TOKEN"))
               for line in contents.splitlines() if "=" in line):
            print("INVALID: ACP gateway bearer tokens must never be written to a committed file", file=sys.stderr)
            return 2
    return 0


def ignored_required(paths: list[str]) -> list[str]:
    """Required files git would silently leave out of every commit.

    A global core.excludesFile that ignores AGENTS.md or .agents/ is a common
    scaffolding default; `git add` then reports success while staging nothing,
    and the project's authority exists on exactly one machine.
    """
    try:
        inside = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--is-inside-work-tree"],
                                capture_output=True, text=True).stdout.strip() == "true"
    except OSError:
        return []
    if not inside:
        return []
    out = subprocess.run(["git", "-C", str(ROOT), "check-ignore", *paths],
                         capture_output=True, text=True)
    return [line for line in out.stdout.splitlines() if line.strip()]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-context", action="store_true")
    args = parser.parse_args()

    project_mode = PROJECT_MODE
    if (ROOT / ".agents").is_symlink():
        print("INVALID: .agents is a symbolic link; bootstrap needs a real .agents/ directory", file=sys.stderr)
        return 2
    required = COMMON + (PROJECT_ONLY if project_mode else REPOSITORY_ONLY)
    if project_mode and acp_removed():
        required = [path for path in required if not path.startswith(".agents/acp/")]
    missing = [path for path in required if not (ROOT / path).exists()]
    if missing:
        print("MISSING:", *missing, sep="\n  ", file=sys.stderr)
        return 2

    readme_issues = readme_findings(ROOT / "README.md")
    marker = json.loads(LAYOUT.path("installation").read_text(encoding="utf-8")) if project_mode else {}
    preserve_existing_readme = project_mode and marker.get("mode") == "existing"
    if readme_issues and preserve_existing_readme:
        print("WARN: existing README preserved; review .agents/bootstrap/docs/README_STANDARD.md:",
              *readme_issues, sep="\n  ")
    elif readme_issues:
        print("INVALID: README does not meet .agents/bootstrap/docs/README_STANDARD.md:",
              *readme_issues, sep="\n  ", file=sys.stderr)
        return 2

    ignored = ignored_required(required + ([".agents/board.env"] if (ROOT / ".agents/board.env").exists() else []))
    if ignored:
        print("INVALID: git ignores required file(s); they would never be committed:",
              *ignored, sep="\n  ", file=sys.stderr)
        print("remedy: add a negation (e.g. '!AGENTS.md' or '!.agents/') to this repository's .gitignore",
              file=sys.stderr)
        return 2

    version = LAYOUT.path("version").read_text(encoding="utf-8").strip()
    if not version or version.startswith("v"):
        print("INVALID: .agents/bootstrap/VERSION must contain bare SemVer", file=sys.stderr)
        return 2
    acp_version = ROOT / ".agents/acp/VERSION"
    if acp_version.exists() and acp_version.read_text(encoding="utf-8").strip() != version:
        print("INVALID: .agents/acp/VERSION and .agents/bootstrap/VERSION must name the same release",
              file=sys.stderr)
        return 2

    json_paths = [f".agents/bootstrap/schemas/{name}" for name in BOOTSTRAP_SCHEMAS[:5]]
    json_paths += [f".agents/acp/schemas/{name}" for name in ACP_SCHEMA_FILES
                   if (ROOT / ".agents/acp/schemas" / name).exists()]
    project_json = [LAYOUT.rel(name) for name in ("profile", "formal_resources", "context_sources", "installation")]
    if project_mode:
        json_paths += project_json
    else:
        json_paths += [f"{TEMPLATES}/{name}" for name in ("profile.json", "formal-resources.json",
                                                          "context-sources.json")]
    for path in json_paths:
        json.loads((ROOT / path).read_text(encoding="utf-8"))

    profile = json.loads(project_file(LAYOUT.rel("profile")).read_text(encoding="utf-8"))
    resources = json.loads(project_file(LAYOUT.rel("formal_resources")).read_text(encoding="utf-8"))
    expected_id = profile.get("projectId")
    if resources.get("projectId") != expected_id:
        print("INVALID: formal resource manifest projectId != profile projectId", file=sys.stderr)
        return 2

    rc = validate_acp(profile)
    if rc:
        return rc

    if project_mode:
        local_items_check = subprocess.call([sys.executable, str(HERE / "validate-local-work-items.py")])
        if local_items_check:
            return local_items_check

    if args.check_context:
        rc = subprocess.call([sys.executable, str(HERE / "gen-context.py"), "--check"])
        if rc:
            return rc

    adapter_check = subprocess.call([sys.executable, str(HERE / "validate-protocol-adapters.py")])
    if adapter_check:
        return adapter_check

    integrity_check = subprocess.call([sys.executable, str(HERE / "check-protocol-integrity.py")], cwd=ROOT)
    if integrity_check:
        return integrity_check

    mode = "project" if project_mode else "repository"
    print(f"OK {mode} structure v{version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
