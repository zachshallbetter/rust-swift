"""Where the ACP client finds a project's files, in contained (0.7+) and legacy layouts.

ACP is standalone: this module does not depend on the bootstrap's own layout
code, and a repository can carry .agents/acp/ without the rest of the bootstrap.
"""
from __future__ import annotations

from pathlib import Path

CLIENT_HOME = Path(__file__).resolve().parents[1]          # .agents/acp
VERSION_FILE = CLIENT_HOME / "VERSION"

CONTAINED_PROFILE = ".agents/project/profile.json"
LEGACY_PROFILE = "PROJECT_PROFILE.json"
MARKERS = (CONTAINED_PROFILE, ".agents/project/installation.json", LEGACY_PROFILE, ".formal-bootstrap.json")

# (contained path, legacy path, kind, required). What an agent would edit to talk
# itself out of a decision: hashed for snapshots, proposed as the policy manifest.
CONTROL_ARTIFACTS = (
    ("AGENTS.md", "AGENTS.md", "control_instruction", True),
    (".agents/bootstrap/NEW_AGENT_PROMPT.md", "NEW_AGENT_PROMPT.md", "control_instruction", False),
    (CONTAINED_PROFILE, LEGACY_PROFILE, "bootstrap", True),
    (".agents/project/intent.md", "PROJECT_INTENT.md", "bootstrap", True),
    (".agents/project/bindings.yaml", "bindings/PROJECT_BINDINGS.yaml", "bootstrap", True),
    (".agents/project/contracts/acp-protected-effects.yaml", "contracts/acp-protected-effects.yaml",
     "policy_manifest", True),
    (".agents/acp/README.md", "docs/ACP_INTEGRATION.md", "control_instruction", True),
    (".agents/board.env", ".agents/board.env", "repository_identity", False),
    (".agents/acp/acp", None, "client_control", True),
    (".agents/acp/lib/acp.py", "scripts/acp.py", "client_control", True),
    (".agents/acp/lib/acp_endpoint_registry.py", "scripts/acp_endpoint_registry.py", "client_control", True),
    (".agents/acp/lib/acp_paths.py", None, "client_control", True),
    (".agents/acp/schemas/acp-endpoint-registry.schema.json", "schemas/acp-endpoint-registry.schema.json",
     "schema", True),
    (".agents/acp/lib/acp-check.py", "scripts/acp-check.py", "client_control", True),
    (".agents/acp/lib/acp_override.py", "scripts/acp_override.py", "client_control", True),
    (".agents/bootstrap/scripts/upgrade-project.py", "scripts/upgrade-project.py", "client_control", True),
    (".agents/bootstrap/scripts/reconcile-project.py", "scripts/reconcile-project.py", "client_control", True),
)
CONTROL_PATTERNS = (
    (".agents/skills/**", ".agents/skills/**", "client_control_tree", "authorize_and_report"),
    (".agents/records/deviations.jsonl", "records/deviations.jsonl", "control_tree", "report_only"),
)


def client_version() -> str:
    try:
        return VERSION_FILE.read_text(encoding="utf-8").strip() or "unknown"
    except OSError:
        return "unknown"


def find_project_root(start: Path | None = None) -> Path:
    """Nearest ancestor holding a project, stopping at the enclosing repository root."""
    here = (start or Path.cwd()).resolve()
    for directory in (here, *here.parents):
        if any((directory / marker).is_file() for marker in MARKERS):
            return directory
        if (directory / ".git").exists():
            return directory
    # No marker and no repository: a tool inside .agents/ belongs to the folder above it.
    for directory in (here, *here.parents):
        if directory.name == ".agents":
            return directory.parent
    return here


def contained(root: Path) -> bool:
    return (root / CONTAINED_PROFILE).is_file() or not (root / LEGACY_PROFILE).is_file()


def profile_path(root: Path) -> Path:
    return root / (CONTAINED_PROFILE if contained(root) else LEGACY_PROFILE)


def records_dir(root: Path) -> Path:
    return root / (".agents/records" if contained(root) else "records")


def local_records_dir(root: Path) -> Path:
    """Git-ignored, per-machine state: the decision log, caches, the policy proposal."""
    return root / (".agents/records/local" if contained(root) else ".formal-bootstrap")


def control_artifacts(root: Path) -> list[tuple[str, str, bool]]:
    """(path, kind, required) in this project's layout; legacy entries without a legacy path are skipped."""
    use_contained = contained(root)
    result = []
    for new, old, kind, required in CONTROL_ARTIFACTS:
        path = new if use_contained else old
        if path:
            result.append((path, kind, required))
    return result


def control_patterns(root: Path) -> list[tuple[str, str, str]]:
    use_contained = contained(root)
    return [(new if use_contained else old, kind, policy) for new, old, kind, policy in CONTROL_PATTERNS]
