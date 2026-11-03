#!/usr/bin/env python3
"""Inspect a repository and report bootstrap adoption state without writing files."""
from __future__ import annotations
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import acp_endpoint_registry

def git(project: Path, *args: str) -> str | None:
    try:
        result = subprocess.run(["git", *args], cwd=project, text=True, capture_output=True, check=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return None
    return result.stdout.strip()


def read_json(project: Path, relative: str) -> dict | None:
    try:
        value = json.loads((project / relative).read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def detect_protocols(project: Path, profile: dict | None) -> list[dict[str, str | None]]:
    found: dict[str, dict[str, str | None]] = {}
    manifest = read_json(project, "FORMAL_RESOURCE_MANIFEST.json")
    if manifest:
        for resource in manifest.get("resources", []):
            if isinstance(resource, dict) and resource.get("id"):
                found[str(resource["id"])] = {
                    "id": str(resource["id"]),
                    "version": resource.get("version"),
                    "revisionOrDigest": resource.get("revisionOrDigest"),
                    "source": resource.get("canonicalSource"),
                    "evidence": "FORMAL_RESOURCE_MANIFEST.json",
                }
    if profile:
        for adapter in profile.get("protocolAdapters", []) or profile.get("profiles", []) or []:
            ident = str(adapter)
            found.setdefault(ident, {"id": ident, "version": None, "revisionOrDigest": None, "source": None, "evidence": "PROJECT_PROFILE.json"})
    iepe_ref = read_json(project, ".iepe/protocol-reference.json")
    if iepe_ref:
        protocol = iepe_ref.get("protocol") if isinstance(iepe_ref.get("protocol"), dict) else iepe_ref
        ident = protocol.get("id") or protocol.get("protocolId") or "iepe"
        found[str(ident)] = {
            "id": str(ident),
            "version": protocol.get("version") or protocol.get("protocolVersion"),
            "revisionOrDigest": protocol.get("revision") or protocol.get("revisionOrDigest"),
            "source": protocol.get("source") or protocol.get("canonicalSource"),
            "evidence": ".iepe/protocol-reference.json",
        }
    return sorted(found.values(), key=lambda item: str(item["id"]))

def detect_acp_integration(project: Path, profile: dict | None, marker: dict | None) -> dict[str, object]:
    evidence_paths = [
        path for path in (
            "scripts/acp.py",
            "scripts/acp-check.py",
            ".agents/board.env",
            "docs/ACP_INTEGRATION.md",
            "ACP_INTEGRATION.md",
        ) if (project / path).is_file()
    ]
    provider = (profile or {}).get("authorizationProvider")
    provider_name = provider.get("name") if isinstance(provider, dict) else None
    bootstrap_owned = bool(
        isinstance(marker, dict)
        and marker.get("bootstrap") == "formal-project-bootstrap"
        and (project / "scripts/acp.py").is_file()
        and (project / "scripts/acp-check.py").is_file()
    )
    if bootstrap_owned:
        status = "formal-project-bootstrap"
        guidance = "Use the bootstrap ACP client and bindings as the canonical project integration."
    elif provider_name == "acp-gateway" or evidence_paths:
        status = "existing-or-legacy"
        guidance = (
            "Preserve the current integration and policy while assessing it. Migrate through existing-project "
            "adoption before replacing it; do not install a parallel ACP authority or client."
        )
    else:
        status = "not-detected"
        guidance = "For a new ACP integration, use Formal Project Bootstrap initialization and policy proposal generation."
    provider = provider if isinstance(provider, dict) else {}
    board = provider.get("board") if isinstance(provider.get("board"), dict) else {}
    endpoint_set = provider.get("gatewayEndpointSet")
    try:
        machine_registry = acp_endpoint_registry.read_registry(
            acp_endpoint_registry.registry_path({"ACP_ENDPOINT_REGISTRY": os.environ.get("ACP_ENDPOINT_REGISTRY", "")})
        )
        endpoint_configured = isinstance(endpoint_set, str) and endpoint_set in machine_registry["endpointSets"]
        endpoint_state = "configured" if endpoint_configured else "missing"
    except (OSError, ValueError) as exc:
        endpoint_state = "invalid" if "invalid" in str(exc).lower() or "unsafe" in str(exc).lower() else "missing"
    repository_identity = (project / ".agents/board.env").is_file() and "ACP_REMOTE_URL=" in (project / ".agents/board.env").read_text(encoding="utf-8")
    board_bound = bool(board.get("owner") and board.get("number"))
    proposal = (project / ".formal-bootstrap/acp-policy-proposal.json").is_file()
    next_steps = []
    if not repository_identity:
        next_steps.append("declare the canonical GitHub repository identity")
    if endpoint_state != "configured":
        next_steps.append("configure the selected machine-local ACP endpoint set and endpoint-specific credentials")
    else:
        next_steps.append(
            "run python3 scripts/acp-check.py --json to verify live credentials, board visibility, and active policy"
        )
    if not board_bound:
        next_steps.append("bind an existing ProjectsV2 board when one is available")
    if not proposal:
        next_steps.append("generate and review an unsigned repository policy proposal")
    return {
        "status": status,
        "provider": provider_name,
        "evidencePaths": evidence_paths,
        "guidance": guidance,
        "setup": {
            "localWiring": "declared" if repository_identity else "missing",
            "endpointSet": {
                "name": endpoint_set,
                "state": endpoint_state,
                "credentialAuthentication": "NOT_VERIFIED",
            },
            "policyProposal": "present" if proposal else "missing",
            "policyRegistration": "NOT_VERIFIED",
            "policyActivation": "NOT_VERIFIED",
            "boardBinding": "bound" if board_bound else "unbound",
            "authorization": "NOT_EVALUATED",
            "projectReadiness": "NOT_EVALUATED",
            "githubReviewDeployment": "NOT_EVALUATED",
            "responsibleRole": "repository owner or ACP project administrator",
            "nextSteps": next_steps,
            "interpretation": "Endpoint-set registration does not verify credential acceptance. Local configuration and proposal state are evidence only; no live authorization or deployment gate was evaluated.",
        },
    }

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", default=".")
    parser.add_argument("--pretty", action="store_true")
    args = parser.parse_args()
    project = Path(args.project).resolve()
    if not project.is_dir():
        print(json.dumps({"error": f"project is not a directory: {project}"}))
        return 2
    findings: list[dict[str, str]] = []
    marker = profile = None
    for label, rel in (("marker", ".formal-bootstrap.json"), ("profile", "PROJECT_PROFILE.json")):
        path = project / rel
        if not path.exists():
            findings.append({"severity": "missing", "path": rel, "meaning": f"bootstrap {label} is absent"})
            continue
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            findings.append({"severity": "error", "path": rel, "meaning": f"bootstrap {label} is unreadable: {exc}"})
            continue
        if label == "marker": marker = value
        else: profile = value
    expected = ["AGENTS.md", ".agents", "CONTEXT_SOURCES.json", ".agents/context-lock.json", "scripts/gen-context.py", "scripts/validate-bootstrap.py", "scripts/validate-local-work-items.py", "scripts/acp.py", "scripts/upgrade-project.py", "scripts/reconcile-project.py", "scripts/validate-protocol-adapters.py", "scripts/validate-qualification.py", "scripts/validate-interaction-contract.py", "scripts/validate-semantic-surface.py", "scripts/recipient-status.py", "scripts/check-protocol-integrity.py", "scripts/esp-instance.py", "scripts/esp-record.py", "scripts/iepe-adopt.py", "schemas", "schemas/protocol-adapter.schema.json", "schemas/qualification-record.schema.json", "schemas/recipient-contract.schema.json", "schemas/interaction-contract.schema.json", "schemas/external-state-instance.schema.json", "schemas/semantic-surface.schema.json", "contracts", "docs/CONFORMANCE.md", "docs/PROTOCOL_ADAPTERS.md"]
    artifacts = {path: ("present" if (project / path).exists() else "missing") for path in expected}
    for path, state in artifacts.items():
        if state == "missing": findings.append({"severity": "missing", "path": path, "meaning": "expected bootstrap or governance artifact is absent"})
    if marker and profile:
        if marker.get("projectId") != profile.get("projectId"): findings.append({"severity": "conflict", "path": ".formal-bootstrap.json/PROJECT_PROFILE.json", "meaning": "project identifiers differ"})
        if marker.get("bootstrapVersion") != profile.get("profileVersion"): findings.append({"severity": "conflict", "path": ".formal-bootstrap.json/PROJECT_PROFILE.json", "meaning": "bootstrap versions differ"})
    resource_ids = []
    if isinstance(profile, dict):
        resource_ids.extend(profile.get("profiles") or [])
    if isinstance(profile, dict) and isinstance(profile.get("formalResourceManifest"), str):
        manifest_path = project / profile["formalResourceManifest"]
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            resource_ids.extend(item.get("id") for item in manifest.get("resources", []) if isinstance(item, dict) and item.get("id"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError):
            pass
    protocol_evidence = []
    for path in (".iepe", ".agents", "PROJECT_PROFILE.json", "FORMAL_RESOURCE_MANIFEST.json", "PROJECT_INTENT.md", "INTENT.md"):
        candidate = project / path
        if candidate.exists():
            protocol_evidence.append({"path": path, "kind": "adoption-or-authority-source"})
    detected_protocols = detect_protocols(project, profile)
    canonical_documents = [
        path for path in ("AGENTS.md", "PROJECT_INTENT.md", "INTENT.md", "AUTHORITY.md", "PROJECT_PROFILE.json", "bindings/PROJECT_BINDINGS.yaml")
        if (project / path).exists()
    ]
    required_records = ["records/evidence.jsonl", "records/negative-results.jsonl", "records/deviations.jsonl", "records/alignment.jsonl", "records/local-work-items.jsonl"]
    if isinstance(profile, dict) and "esp-rcp" in (profile.get("protocolAdapters") or profile.get("profiles") or []):
        required_records.extend(["records/campaigns.jsonl", "records/commitments.jsonl", "records/qualifications.jsonl", "records/receipts.jsonl", "records/measurements.jsonl", "records/learning.jsonl", "records/external-state-events.jsonl", "records/recipient.json", "records/external-state.json"])
    missing_required_records = [path for path in required_records if not (project / path).exists()]
    detected_by_id = {str(item["id"]): item for item in detected_protocols}
    version_revision_drift: list[dict[str, str | None]] = []
    unsupported_protocol_features: list[dict[str, object]] = []
    for adapter_id in (profile or {}).get("protocolAdapters", []) if isinstance(profile, dict) else []:
        adapter = read_json(project, f"profiles/{adapter_id}/adapter.json")
        if not adapter:
            version_revision_drift.append({"adapter": str(adapter_id), "meaning": "adapter declaration is missing or unreadable"})
            continue
        protocol = adapter.get("protocol") if isinstance(adapter.get("protocol"), dict) else {}
        ident = str(protocol.get("id") or adapter_id)
        detected = detected_by_id.get(ident) or detected_by_id.get(str(adapter_id))
        if not detected:
            version_revision_drift.append({"adapter": str(adapter_id), "meaning": f"pinned protocol {ident} is not declared in project evidence"})
        else:
            for field, expected in (("version", protocol.get("version")), ("revisionOrDigest", protocol.get("revisionOrDigest"))):
                if expected and detected.get(field) and detected.get(field) != expected:
                    version_revision_drift.append({"adapter": str(adapter_id), "field": field, "expected": str(expected), "observed": str(detected.get(field))})
        unsupported_protocol_features.append({"adapter": str(adapter_id), "protocol": ident, "declaredUnsupported": adapter.get("unsupported", [])})
    if (project / "INTENT.md").exists() and (project / "PROJECT_INTENT.md").exists():
        findings.append({"severity": "review", "path": "INTENT.md/PROJECT_INTENT.md", "meaning": "two intent sources exist; authority precedence must be declared"})
    if (project / ".iepe").exists() and (project / "PROJECT_PROFILE.json").exists() and not (profile or {}).get("protocolAdapters"):
        findings.append({"severity": "review", "path": ".iepe/PROJECT_PROFILE.json", "meaning": "IEPE state exists without an explicit FPB protocol adapter binding"})
    context_freshness = "not-applicable"
    context_script = project / "scripts/gen-context.py"
    if context_script.exists() and (project / ".agents/context-lock.json").exists():
        try:
            check = subprocess.run([sys.executable, str(context_script), "--check"], cwd=project, text=True, capture_output=True, timeout=30)
            context_freshness = "fresh" if check.returncode == 0 else "stale-or-invalid"
        except (OSError, subprocess.SubprocessError):
            context_freshness = "unavailable"
    authority_conflicts = [item for item in findings if item["severity"] in {"conflict", "review"}]
    recommended_migration_steps = []
    if missing_required_records: recommended_migration_steps.append("create missing required ledgers or records after authority review")
    if version_revision_drift: recommended_migration_steps.append("reconcile protocol pins and re-run conformance checks")
    if authority_conflicts: recommended_migration_steps.append("resolve authority overlaps before copying or promoting any canonical document")
    if context_freshness != "fresh": recommended_migration_steps.append("refresh generated context only after source authority is settled")
    acp_integration = detect_acp_integration(project, profile, marker if isinstance(marker, dict) else None)
    result = {"format": "1", "project": str(project), "git": {"root": git(project, "rev-parse", "--show-toplevel"), "head": git(project, "rev-parse", "HEAD"), "branch": git(project, "branch", "--show-current"), "status": git(project, "status", "--short"), "remote": git(project, "config", "--get", "remote.origin.url")}, "bootstrap": {"initialized": marker is not None and profile is not None, "marker": marker, "profile": profile, "artifacts": artifacts, "declaredProtocolResources": sorted(set(resource_ids)), "protocolEvidence": protocol_evidence, "detectedProtocols": detected_protocols, "canonicalDocuments": canonical_documents, "missingRequiredRecords": missing_required_records, "versionRevisionDrift": version_revision_drift, "unsupportedProtocolFeatures": unsupported_protocol_features, "contextFreshness": context_freshness}, "acpIntegration": acp_integration, "authorityConflicts": authority_conflicts, "recommendedMigrationSteps": recommended_migration_steps, "findings": findings}
    result["reconcileRequired"] = any(item["severity"] in {"missing", "conflict", "error"} for item in findings)
    print(json.dumps(result, indent=2 if args.pretty else None, sort_keys=True))
    return 1 if result["reconcileRequired"] else 0

if __name__ == "__main__":
    raise SystemExit(main())
