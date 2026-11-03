# Standards & Core Operating Doctrine

## 1. Epistemic Integrity and Status Honesty

Always enforce the strict Four-Tier Claim Taxonomy:

$$\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$$

- **Never Conflate Maturity**: A feature documented in an RFC or spec is documented, not implemented. A function with passing unit tests is tested, not empirically validated. A benchmark written alongside its implementation is a fixture, not an empirical proof. Always state the weaker claim when in doubt.
- **Permanent Retention of Negative Results**: Abandoned hypotheses, discredited metrics, and disproven architectural directions must be recorded permanently in research logs (`docs/NEGATIVE_RESULTS.md`). Never prune disproven paths: preserving the record prevents re-proposing abandoned intuitions.
- **Prospective Pre-Registration**: Define success criteria, baselines, and quantitative invalidation triggers before running experiments or benchmarks.

## 2. Hierarchy of Authority

When documentation, code, generated artifacts, or AI outputs conflict, resolve in strict canonical order per `AUTHORITY.md`:

$$\text{Architecture & Doctrine} \to \text{Ontology & Glossary} \to \text{Formal Specification} \to \text{Substrate Code} \to \text{Generated Artifacts}$$

Code, generated artifacts, or model outputs never silently alter theory or formal specifications. If code disagrees with specification, the code contains a defect until a formal synthesis determination updates the specification.

## 3. Root Cause Investigation

- Inspect full, un-truncated error tracebacks and runtime logs before diagnosing failures.
- Never resolve errors by swallowing exceptions, masking symptoms, commenting out broken assertions, returning dummy fallbacks, or deleting failing unit tests. Always trace to and resolve the upstream root cause.

## 4. Non-Destructive Engineering and Preservation

- Preserve existing comments, docstrings, type annotations, and unrelated functionality unless explicitly directed to refactor.
- Surgical Edits: Prefer precise, targeted code modifications over full-file overwrites to minimize diff noise and prevent accidental regressions.

## 5. Layer Independence and Decoupled Architecture

- Independent Versioning: Theory, core engines, runtime adapters, and examples move on independent version paths with per-layer changelogs. Never infer cross-layer conformance from matching version numbers.
- Zero-Dependency Cores: Keep core mathematical and domain logic decoupled from external I/O, UI frameworks, or rendering layers. Provide renderer-agnostic, signals-first default modes.

## 6. Agent-First Publishing Architecture

- Generate `llms.txt` manifests and `llms-full.txt` context corpora locally when full-text agent search is needed. Keep generated exports ignored and out of Git commits and release archives.
- Downstream invariant: generated context is a projection, never authority. Do not edit it manually; refresh it with `.agents/bootstrap/scripts/gen-context.py`. The source-bound `.agents/context-lock.json` remains the freshness record.

## 7. Engineering Voice and Attribution

- Clean, human engineering prose.
- Zero `Co-Authored-By` trailers, no generated-by-AI footers, and no agent session identifiers in tracked code, documentation, or commits.

## 8. Research Paper & Monograph Directory Standard (RPDS)

Where a program or project produces a formal research paper, monograph, or scholarly deposit:

- **Canonical Directory Layout**: Organize under `paper/` (or `docs/research/`) with modular `sections/` (`00` status matrix through `08` conclusions), reproducible `figures/`, archival `latex/`, and `supplementary/`.
- **Dual-Format Delivery Mandate**: Every paper (monograph and supplementary papers alike) must exist simultaneously as an inspectable Markdown file (`.md`) and a publication-grade compiled PDF (`.pdf`) compiled via XeLaTeX.
- **Self-Documenting Naming Discipline**: Never use generic filenames like `paper.md` or `paper.pdf`. All paper deliverables must use semantic slugs matching `{stem}.md` and `{stem}.pdf` (e.g. `[project]-[component]-[version].{md,pdf}`).
- **Sub-Research Paper Invariant**: Supplementary papers are formal research monographs requiring YAML frontmatter, decimal numbered headings, formal model, falsifiability criteria, verified bibliography, and compiled matching PDF.
- **OpenAIRE & Zenodo FAIR Compliance**: Maintain synchronized `.zenodo.json` and `CITATION.cff` metadata with author ORCID (`0009-0009-9450-3429`), concept and version DOIs, and typed `related_identifiers`.
- **Automated Verification**: Every paper package must verify with `rdp-check.py --paper-dir <dir>` with zero errors and zero warnings.
