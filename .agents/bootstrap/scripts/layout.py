"""Where every bootstrap and ACP file lives.

Since 0.7.0 a project keeps everything under `.agents/` and only `AGENTS.md` at
its root. Projects created before then keep the legacy layout until
`bootstrap upgrade` moves them; every reader goes through `Layout`, so both work.

One table, `PATHS`, is the single source of truth: the layout readers, the
0.7.0 migration, the AGENTS.md path rewrite and the external-reference report
all derive from it.
"""
from __future__ import annotations

from pathlib import Path

# name: (contained path, legacy path)
PATHS: dict[str, tuple[str, str]] = {
    "installation": (".agents/project/installation.json", ".formal-bootstrap.json"),
    "intent": (".agents/project/intent.md", "PROJECT_INTENT.md"),
    "profile": (".agents/project/profile.json", "PROJECT_PROFILE.json"),
    "context_sources": (".agents/project/context-sources.json", "CONTEXT_SOURCES.json"),
    "formal_resources": (".agents/project/formal-resources.json", "FORMAL_RESOURCE_MANIFEST.json"),
    "bindings": (".agents/project/bindings.yaml", "bindings/PROJECT_BINDINGS.yaml"),
    "contracts": (".agents/project/contracts", "contracts"),
    "rules": (".agents/project/rules", ".agents/rules"),
    "records": (".agents/records", "records"),
    "local_records": (".agents/records/local", ".formal-bootstrap"),
    "policy_proposal": (".agents/records/local/acp-policy-proposal.json",
                        ".formal-bootstrap/acp-policy-proposal.json"),
    "upgrades": (".agents/.upgrades", ".formal-bootstrap/upgrades"),
    "candidates": (".agents/.upgrades/candidates", ".formal-bootstrap/candidates"),
    "version": (".agents/bootstrap/VERSION", "VERSION"),
    "new_agent_prompt": (".agents/bootstrap/NEW_AGENT_PROMPT.md", "NEW_AGENT_PROMPT.md"),
    "bootstrap_scripts": (".agents/bootstrap/scripts", "scripts"),
    "bootstrap_schemas": (".agents/bootstrap/schemas", "schemas"),
    "bootstrap_docs": (".agents/bootstrap/docs", "docs"),
    "protocol_profiles": (".agents/bootstrap/profiles", "profiles"),
    "acp_lib": (".agents/acp/lib", "scripts"),
    "acp_schemas": (".agents/acp/schemas", "schemas"),
    "acp_readme": (".agents/acp/README.md", "docs/ACP_INTEGRATION.md"),
    "env_example": (".agents/acp/env.example", ".env.example"),
    "board_env": (".agents/board.env", ".agents/board.env"),
    "skills": (".agents/skills", ".agents/skills"),
    "context_lock": (".agents/context-lock.json", ".agents/context-lock.json"),
}

# The ledgers bootstrap creates. A project's own files in a legacy records/
# (benchmarks, sync logs) are never moved.
RECORD_FILES = ("evidence.jsonl", "negative-results.jsonl", "deviations.jsonl", "alignment.jsonl",
                "local-work-items.jsonl")
PROTOCOL_RECORD_FILES = ("campaigns.jsonl", "commitments.jsonl", "qualifications.jsonl", "receipts.jsonl",
                         "measurements.jsonl", "learning.jsonl", "external-state-events.jsonl",
                         "external-state.json", "recipient.json")
ACP_LIB_FILES = ("acp.py", "acp_endpoint_registry.py", "acp_override.py", "acp-check.py")
ACP_SCHEMA_FILES = ("acp-decision.schema.json", "acp-endpoint-registry.schema.json")
BOOTSTRAP_SCRIPT_FILES = ("gen-context.py", "validate-bootstrap.py", "validate-local-work-items.py", "upgrade-project.py",
                          "reconcile-project.py", "validate-protocol-adapters.py", "validate-qualification.py",
                          "validate-interaction-contract.py", "validate-semantic-surface.py", "recipient-status.py",
                          "check-protocol-integrity.py", "esp-instance.py", "esp-record.py", "iepe-adopt.py",
                          "init-project.py", "package-release.py", "check-all.py", "validate-formal-files.py",
                          "check-negative-results.py", "release-manifest.py")

# Directories whose content a release owns and replaces. Everything else under
# .agents/ belongs to the project or to other tools and is never touched.
MACHINERY_DIRS = (".agents/acp", ".agents/bootstrap")
PROJECT_OWNED_DIRS = (".agents/project", ".agents/records")

# Repository-only paths inside a release checkout: templates are rendered into
# .agents/project/ by init and upgrade.
TEMPLATES = ".agents/bootstrap/templates"
REPOSITORY_ROOT = Path(__file__).resolve().parents[3]

PROJECT_MARKERS = (PATHS["installation"][0], PATHS["profile"][0], PATHS["installation"][1],
                   PATHS["profile"][1])


def find_project_root(start: Path | None = None) -> Path:
    """The nearest ancestor that holds a bootstrap project, never crossing a repository boundary.

    Nested projects (a submodule that is itself bootstrapped) and agent worktrees
    under .claude/worktrees/ resolve to themselves, not to the repository that
    contains them.
    """
    here = (start or Path.cwd()).resolve()
    for directory in (here, *here.parents):
        if any((directory / marker).is_file() for marker in PROJECT_MARKERS):
            return directory
        if (directory / ".git").exists():
            return directory
    # No marker and no repository: a tool inside .agents/ belongs to the folder above it.
    for directory in (here, *here.parents):
        if directory.name == ".agents":
            return directory.parent
    return here


class Layout:
    """Paths for one project, contained or legacy."""

    def __init__(self, root: Path, contained: bool | None = None):
        self.root = Path(root)
        if contained is None:
            contained = (self.root / PATHS["installation"][0]).is_file() or \
                not (self.root / PATHS["installation"][1]).is_file()
        self.contained = contained

    def rel(self, name: str) -> str:
        contained, legacy = PATHS[name]
        return contained if self.contained else legacy

    def path(self, name: str) -> Path:
        return self.root / self.rel(name)

    def record(self, filename: str) -> Path:
        return self.path("records") / filename

    def acp_file(self, filename: str) -> Path:
        return self.path("acp_lib") / filename

    def bootstrap_script(self, filename: str) -> Path:
        return self.path("bootstrap_scripts") / filename

    def doc(self, filename: str) -> Path:
        if filename == "ACP_INTEGRATION.md":
            return self.path("acp_readme")
        return self.path("bootstrap_docs") / filename

    def schema(self, filename: str) -> Path:
        name = "acp_schemas" if filename in ACP_SCHEMA_FILES else "bootstrap_schemas"
        return self.path(name) / filename


def migration_moves(docs: tuple[str, ...], scripts: tuple[str, ...], schemas: tuple[str, ...]) -> list[tuple[str, str]]:
    """(legacy, contained) for every path the 0.7.0 migration moves, most specific first."""
    moves = [(legacy, contained) for name, (contained, legacy) in PATHS.items()
             if contained != legacy and name not in ("bootstrap_scripts", "bootstrap_schemas", "bootstrap_docs",
                                                     "acp_lib", "acp_schemas", "records", "local_records")]
    for filename in RECORD_FILES + PROTOCOL_RECORD_FILES:
        moves.append((f"records/{filename}", f".agents/records/{filename}"))
    for filename in scripts:
        target = PATHS["acp_lib"][0] if filename in ACP_LIB_FILES else PATHS["bootstrap_scripts"][0]
        moves.append((f"scripts/{filename}", f"{target}/{filename}"))
    for filename in schemas:
        target = PATHS["acp_schemas"][0] if filename in ACP_SCHEMA_FILES else PATHS["bootstrap_schemas"][0]
        moves.append((f"schemas/{filename}", f"{target}/{filename}"))
    for filename in docs:
        if filename != "ACP_INTEGRATION.md":
            moves.append((f"docs/{filename}", f"{PATHS['bootstrap_docs'][0]}/{filename}"))
    return sorted(set(moves), key=lambda move: (-len(move[0]), move[0]))


# Repository-only tools a release does not install into projects.
REPOSITORY_ONLY = frozenset({
    ".agents/bootstrap/scripts/init-project.py", ".agents/bootstrap/scripts/package-release.py",
    ".agents/bootstrap/scripts/check-all.py", ".agents/bootstrap/scripts/validate-formal-files.py",
    ".agents/bootstrap/scripts/check-negative-results.py", ".agents/bootstrap/scripts/release-manifest.py",
})
SHIPPED_SKILLS = ".agents/skills"


def release_machinery(release: Path) -> dict[str, bytes]:
    """Every machinery file a release installs, at its installed path (source = install)."""
    files: dict[str, bytes] = {}
    for top in (*MACHINERY_DIRS, SHIPPED_SKILLS):
        base = release / top
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            rel = path.relative_to(release).as_posix()
            if (not path.is_file() or path.is_symlink() or "__pycache__" in path.parts
                    or path.suffix in (".pyc", ".pyo") or rel in REPOSITORY_ONLY or path.name == ".DS_Store"):
                continue
            files[rel] = path.read_bytes()
    return files


SHIPPED_DOCS = ("AGENT_CAPABILITIES.md", "ALIGNMENT.md", "AUTHORITY_MODEL.md", "BOOTSTRAP_SEQUENCE.md",
                "CONFORMANCE.md", "CONTEXT_COMPILATION.md", "EVIDENCE_METHOD.md", "EXECUTION_PIPELINE.md",
                "EXECUTION_PROVIDER_CONTRACT.md", "EXISTING_PROJECT_ADOPTION.md", "UPGRADING.md",
                "README_STANDARD.md", "ACP_INTEGRATION.md", "FORMAL_RESOURCES.md", "INITIAL_EPICS_TEMPLATE.md",
                "LANDING_AND_PROMOTION.md", "MULTI_AGENT_COORDINATION.md", "NEGATIVE_KNOWLEDGE.md", "RECOVERY.md",
                "SKILLS_INDEX.md", "WORK_GRAPH.md", "PROTOCOL_ADAPTERS.md")
SHIPPED_SCHEMAS = ("agent-message.schema.json", "context-lock.schema.json", "context-sources.schema.json",
                   "formal-resource-manifest.schema.json", "work-disposition.schema.json",
                   "protocol-adapter.schema.json", "qualification-record.schema.json",
                   "recipient-contract.schema.json", "interaction-contract.schema.json",
                   "external-state-instance.schema.json", "semantic-surface.schema.json", "local-work-item.schema.json")


def text_rewrites() -> list[tuple[str, str]]:
    """(legacy text, contained text) for paths quoted in prose, commands and JSON values, most specific first.

    Only exact, known paths are rewritten: a bare `docs/` or `records/` may be the
    project's own and is left alone.
    """
    rewrites = [
        ("python3 scripts/acp-check.py", ".agents/acp/acp status"),
        ("python3 scripts/acp.py", ".agents/acp/acp"),
        ("scripts/acp.py", ".agents/acp/lib/acp.py"),
        ("scripts/acp-check.py", ".agents/acp/lib/acp-check.py"),
        ("scripts/acp_endpoint_registry.py", ".agents/acp/lib/acp_endpoint_registry.py"),
        ("scripts/acp_override.py", ".agents/acp/lib/acp_override.py"),
        ("docs/ACP_INTEGRATION.md", ".agents/acp/README.md"),
        ("bindings/PROJECT_BINDINGS.yaml", ".agents/project/bindings.yaml"),
        ("PROJECT_PROFILE.json", ".agents/project/profile.json"),
        ("PROJECT_INTENT.md", ".agents/project/intent.md"),
        ("CONTEXT_SOURCES.json", ".agents/project/context-sources.json"),
        ("FORMAL_RESOURCE_MANIFEST.json", ".agents/project/formal-resources.json"),
        ("NEW_AGENT_PROMPT.md", ".agents/bootstrap/NEW_AGENT_PROMPT.md"),
        (".formal-bootstrap.json", ".agents/project/installation.json"),
        (".formal-bootstrap/acp-policy-proposal.json", ".agents/records/local/acp-policy-proposal.json"),
        (".formal-bootstrap/upgrades", ".agents/.upgrades"),
        (".formal-bootstrap/candidates", ".agents/.upgrades/candidates"),
        ("contracts/acp-protected-effects.yaml", ".agents/project/contracts/acp-protected-effects.yaml"),
        (".agents/rules/", ".agents/project/rules/"),
    ]
    for name in BOOTSTRAP_SCRIPT_FILES:
        rewrites.append((f"scripts/{name}", f".agents/bootstrap/scripts/{name}"))
    for name in ACP_SCHEMA_FILES:
        rewrites.append((f"schemas/{name}", f".agents/acp/schemas/{name}"))
    for name in SHIPPED_SCHEMAS:
        rewrites.append((f"schemas/{name}", f".agents/bootstrap/schemas/{name}"))
    for name in SHIPPED_DOCS:
        if name != "ACP_INTEGRATION.md":
            rewrites.append((f"docs/{name}", f".agents/bootstrap/docs/{name}"))
    for name in RECORD_FILES + PROTOCOL_RECORD_FILES:
        rewrites.append((f"records/{name}", f".agents/records/{name}"))
    return sorted(rewrites, key=lambda pair: -len(pair[0]))


def rewrite_paths(text: str, extra: tuple[tuple[str, str], ...] = ()) -> tuple[str, int]:
    """Replace legacy paths with contained ones; (new text, replacements). Idempotent."""
    import re
    count = 0
    for old, new in sorted((*text_rewrites(), *extra), key=lambda pair: -len(pair[0])):
        # Not part of a longer path, and not already the contained form.
        pattern = re.compile(r"(?<![\w./-])" + re.escape(old) + r"(?![\w-])")
        text, n = pattern.subn(new, text)
        count += n
    return text, count


def contained_path(legacy_rel: str) -> str | None:
    """Where a legacy-layout file lives once contained, or None when it is not bootstrap's to move."""
    exact = {legacy: contained for name, (contained, legacy) in PATHS.items()
             if contained != legacy and not name.endswith(("scripts", "schemas", "docs", "lib", "records"))
             and name not in ("contracts", "rules", "local_records", "upgrades", "candidates", "protocol_profiles")}
    if legacy_rel in exact:
        return exact[legacy_rel]
    head, _, tail = legacy_rel.partition("/")
    if head == "scripts" and "/" not in tail:
        if tail in ACP_LIB_FILES:
            return f".agents/acp/lib/{tail}"
        if tail in BOOTSTRAP_SCRIPT_FILES:
            return f".agents/bootstrap/scripts/{tail}"
    if head == "schemas" and "/" not in tail:
        if tail in ACP_SCHEMA_FILES:
            return f".agents/acp/schemas/{tail}"
        if tail in SHIPPED_SCHEMAS:
            return f".agents/bootstrap/schemas/{tail}"
    if head == "docs" and tail in SHIPPED_DOCS:
        return PATHS["acp_readme"][0] if tail == "ACP_INTEGRATION.md" else f".agents/bootstrap/docs/{tail}"
    if head == "records" and tail in RECORD_FILES + PROTOCOL_RECORD_FILES:
        return f".agents/records/{tail}"
    if head == "contracts" and tail:
        return f".agents/project/contracts/{tail}"
    if legacy_rel.startswith(".agents/rules/"):
        return ".agents/project/rules/" + legacy_rel[len(".agents/rules/"):]
    if head == "profiles" and tail:
        return f".agents/bootstrap/profiles/{tail}"
    return None


LEGACY_DIRECTORIES = ("scripts", "schemas", "docs", "bindings", "contracts", "records", "profiles",
                      ".formal-bootstrap", ".agents/rules")


RELEASE_MANIFEST = ".agents/bootstrap/RELEASE.json"


def release_manifest(release: Path) -> dict:
    """{version, files: {path: sha256}} for every machinery file a release installs."""
    import hashlib
    files = {rel: hashlib.sha256(content).hexdigest()
             for rel, content in release_machinery(release).items() if rel != RELEASE_MANIFEST}
    version = (release / PATHS["version"][0]).read_text(encoding="utf-8").strip()
    return {"format": 1, "bootstrap": "formal-project-bootstrap", "version": version, "files": dict(sorted(files.items()))}
