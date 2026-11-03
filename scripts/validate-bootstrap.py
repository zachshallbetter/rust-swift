#!/usr/bin/env python3
"""Validate either the bootstrap repository or an initialized project."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMMON = [
    "VERSION",
    "AGENTS.md",
    "docs/AUTHORITY_MODEL.md",
    "docs/ALIGNMENT.md",
    "docs/BOOTSTRAP_SEQUENCE.md",
    "docs/CONTEXT_COMPILATION.md",
    "docs/UPGRADING.md",
    "docs/README_STANDARD.md",
    "docs/EVIDENCE_METHOD.md",
    "docs/EXECUTION_PIPELINE.md",
    "docs/EXECUTION_PROVIDER_CONTRACT.md",
    "docs/ACP_INTEGRATION.md",
    "docs/FORMAL_RESOURCES.md",
    "docs/LANDING_AND_PROMOTION.md",
    "docs/MULTI_AGENT_COORDINATION.md",
    "docs/NEGATIVE_KNOWLEDGE.md",
    "docs/RECOVERY.md",
    "docs/SKILLS_INDEX.md",
    "docs/WORK_GRAPH.md",
    "schemas/agent-message.schema.json",
    "schemas/acp-decision.schema.json",
    "schemas/context-lock.schema.json",
    "schemas/context-sources.schema.json",
    "schemas/formal-resource-manifest.schema.json",
    "schemas/work-disposition.schema.json",
    "schemas/protocol-adapter.schema.json",
    "schemas/qualification-record.schema.json",
    "schemas/recipient-contract.schema.json",
    "schemas/interaction-contract.schema.json",
    "schemas/external-state-instance.schema.json",
    "schemas/semantic-surface.schema.json",
    "contracts/acp-protected-effects.yaml",
    ".agents/skills/authorize-protected-effect/SKILL.md",
    ".agents/skills/github-projects/SKILL.md",
    ".agents/rules/standards.md",
    ".agents/rules/workflow.md",
    ".agents/rules/codestyle.md",
    ".agents/rules/types.md",
    ".agents/rules/ci.md",
    ".agents/rules/governance.md",
    ".agents/rules/security.md",
    "scripts/gen-context.py",
    "scripts/upgrade-project.py",
    "scripts/reconcile-project.py",
    "scripts/acp-check.py",
    "scripts/validate-local-work-items.py",
    "scripts/acp.py",
    "scripts/acp_override.py",
    "scripts/validate-protocol-adapters.py",
    "scripts/validate-qualification.py",
    "scripts/validate-interaction-contract.py",
    "scripts/recipient-status.py",
    "scripts/check-protocol-integrity.py",
    "scripts/esp-instance.py",
    "scripts/esp-record.py",
    "scripts/iepe-adopt.py",
    "scripts/validate-semantic-surface.py",
]
ACP_DECISIONS = {"ALLOW", "DENY", "AUTH_REQUIRED", "REVERIFY_REQUIRED", "QUARANTINE",
                 "LOCKED", "RECOVERY_AUTHORIZED", "VERIFY_RECOVERY"}
REPOSITORY_ONLY = [
    "README.md",
    "AUTHORITY.md",
    "VERSIONING.md",
    "GOVERNANCE.md",
    "PROJECT_INTENT_TEMPLATE.md",
    "PROJECT_PROFILE_TEMPLATE.json",
    "FORMAL_RESOURCE_MANIFEST_TEMPLATE.json",
    "CONTEXT_SOURCES_TEMPLATE.json",
    "bindings/PROJECT_BINDINGS_TEMPLATE.yaml",
    "scripts/init-project.py",
    "scripts/package-release.py",
]
PROJECT_ONLY = [
    "README.md",
    ".formal-bootstrap.json",
    "PROJECT_INTENT.md",
    "PROJECT_PROFILE.json",
    "FORMAL_RESOURCE_MANIFEST.json",
    "CONTEXT_SOURCES.json",
    "bindings/PROJECT_BINDINGS.yaml",
    "records/evidence.jsonl",
    "records/negative-results.jsonl",
    "records/deviations.jsonl",
    "records/alignment.jsonl",
    "records/local-work-items.jsonl",
]


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


def validate_acp(profile: dict) -> int:
    acp = profile.get("authorizationProvider")
    if not isinstance(acp, dict):
        print("INVALID: profile must declare authorizationProvider (see docs/ACP_INTEGRATION.md)", file=sys.stderr)
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
    for key in ("integrationRef", "decisionSchema", "actionMap", "checkScript"):
        ref = acp.get(key)
        if not ref or not (ROOT / ref).exists():
            print(f"INVALID: authorizationProvider.{key} must reference an existing file ({ref})", file=sys.stderr)
            return 2
    # Optional until every adopting project has re-pinned to the current release.
    client = acp.get("clientScript")
    if client is not None and not (ROOT / client).exists():
        print(f"INVALID: authorizationProvider.clientScript must reference an existing file ({client})", file=sys.stderr)
        return 2
    schema = json.loads((ROOT / acp["decisionSchema"]).read_text(encoding="utf-8"))
    decisions = set(schema["properties"]["decision"]["enum"])
    if decisions != ACP_DECISIONS:
        print("INVALID: acp-decision schema decision vocabulary drifted from A0-A7", file=sys.stderr)
        return 2
    action_map = (ROOT / acp["actionMap"]).read_text(encoding="utf-8")
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

    project_mode = (ROOT / ".formal-bootstrap.json").exists()
    required = COMMON + (PROJECT_ONLY if project_mode else REPOSITORY_ONLY)
    missing = [path for path in required if not (ROOT / path).exists()]
    if missing:
        print("MISSING:", *missing, sep="\n  ", file=sys.stderr)
        return 2

    readme_issues = readme_findings(ROOT / "README.md")
    marker = json.loads((ROOT / ".formal-bootstrap.json").read_text(encoding="utf-8")) if project_mode else {}
    preserve_existing_readme = project_mode and marker.get("mode") == "existing"
    if readme_issues and preserve_existing_readme:
        print("WARN: existing README preserved; review docs/README_STANDARD.md:",
              *readme_issues, sep="\n  ")
    elif readme_issues:
        print("INVALID: README does not meet docs/README_STANDARD.md:",
              *readme_issues, sep="\n  ", file=sys.stderr)
        return 2

    ignored = ignored_required(required + ([".agents/board.env"] if (ROOT / ".agents/board.env").exists() else []))
    if ignored:
        print("INVALID: git ignores required file(s); they would never be committed:",
              *ignored, sep="\n  ", file=sys.stderr)
        print("remedy: add a negation (e.g. '!AGENTS.md') to this repository's .gitignore", file=sys.stderr)
        return 2

    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    if not version or version.startswith("v"):
        print("INVALID: VERSION must contain bare SemVer", file=sys.stderr)
        return 2

    json_paths = [
        "schemas/agent-message.schema.json",
        "schemas/acp-decision.schema.json",
        "schemas/context-lock.schema.json",
        "schemas/context-sources.schema.json",
        "schemas/formal-resource-manifest.schema.json",
        "schemas/work-disposition.schema.json",
    ]
    if project_mode:
        json_paths += [
            "PROJECT_PROFILE.json",
            "FORMAL_RESOURCE_MANIFEST.json",
            "CONTEXT_SOURCES.json",
            ".formal-bootstrap.json",
        ]
    else:
        json_paths += [
            "PROJECT_PROFILE_TEMPLATE.json",
            "FORMAL_RESOURCE_MANIFEST_TEMPLATE.json",
            "CONTEXT_SOURCES_TEMPLATE.json",
        ]

    for path in json_paths:
        json.loads((ROOT / path).read_text(encoding="utf-8"))

    profile_path = ROOT / ("PROJECT_PROFILE.json" if project_mode else "PROJECT_PROFILE_TEMPLATE.json")
    profile = json.loads(profile_path.read_text(encoding="utf-8"))
    if profile.get("profileVersion") != version:
        print("INVALID: project profile version != VERSION", file=sys.stderr)
        return 2

    resource_path = ROOT / ("FORMAL_RESOURCE_MANIFEST.json" if project_mode else "FORMAL_RESOURCE_MANIFEST_TEMPLATE.json")
    resources = json.loads(resource_path.read_text(encoding="utf-8"))
    expected_id = profile.get("projectId")
    if resources.get("projectId") != expected_id:
        print("INVALID: formal resource manifest projectId != profile projectId", file=sys.stderr)
        return 2

    rc = validate_acp(profile)
    if rc:
        return rc

    if project_mode:
        local_items_check = subprocess.call([sys.executable, str(ROOT / "scripts/validate-local-work-items.py")])
        if local_items_check:
            return local_items_check

    if args.check_context:
        rc = subprocess.call([sys.executable, str(ROOT / "scripts/gen-context.py"), "--check"])
        if rc:
            return rc

    adapter_check = subprocess.call([sys.executable, str(ROOT / "scripts/validate-protocol-adapters.py")])
    if adapter_check:
        return adapter_check

    integrity_check = subprocess.call([sys.executable, str(ROOT / "scripts/check-protocol-integrity.py")])
    if integrity_check:
        return integrity_check

    mode = "project" if project_mode else "repository"
    print(f"OK {mode} structure v{version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
