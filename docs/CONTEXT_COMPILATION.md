# Context Compilation

The source-bound context lock is required operating infrastructure after project initialization. Full-text exports are generated locally when needed and are never normative authority.

## Outputs

```text
.agents/llms.txt
.agents/llms-full.txt
.agents/context-lock.json
```

`llms.txt` is the search/navigation index and `llms-full.txt` is the deterministic transport/search corpus. Both are optional local outputs ignored by Git and excluded from release archives. `context-lock.json` binds the selected source contents and records Git repository state observed when compilation occurred; it is the validation input even when corpus exports are absent.

The compiler validates its source policy, ignores symlinks and sensitive-file patterns, rejects unreadable or changing eligible sources, and redacts credentials from Git remote provenance. Exclusions are reported during generation. Generated artifacts are published only after successful compilation; `--check` is read-only.

## Validity

Context validity is determined by the aggregate digest of selected canonical sources. Repository HEAD is retained as **provenance**, not used as a self-referential validity predicate: a tracked generated file cannot contain the hash of the commit that contains itself.

A changed branch or commit therefore triggers reconciliation, not automatic semantic invalidity. If selected source contents changed, the digest changes and the context is stale. If Git state changed but selected sources did not, the corpus remains content-valid, while branch-specific/absence claims must still be rechecked against live repository state.

## Rules

- Never edit generated context to fix authority.
- Regenerate after material canonical-source changes.
- Record exact repository/nested-repository state before consequential work.
- Treat null search results as bounded by the checkout represented, not as universal absence.
- Active workers load only issue-relevant excerpts even though the full corpus is available.

## Commands

```bash
python3 scripts/gen-context.py
python3 scripts/gen-context.py --check
# Optional bounded compilation:
python3 scripts/gen-context.py --path docs --path scripts/gen-context.py
# Compile the commit being made rather than the working tree:
python3 scripts/gen-context.py --staged
# Do that on every commit, and stage the lock with it:
python3 scripts/gen-context.py --install-hook
```

In a Git work tree the compiler reads only files Git tracks or would add, so ignored local files never reach the corpus. It still reads their working-tree content, so an uncommitted edit to a tracked file changes the lock. `--staged` reads membership and content from the index instead. The lock it writes is what a clean checkout of the resulting commit computes, whatever else is uncommitted. `--install-hook` writes a pre-commit hook that runs `--staged` and stages `.agents/context-lock.json`, plus the corpus exports where they are tracked. It never replaces a pre-commit hook it did not write, and it refuses a `core.hooksPath` outside the repository. Bypass it for one commit with `SKIP_CONTEXT_HOOK=1`.

The source policy may add `excludePatterns` for project-specific sensitive files. The compiler always excludes common build/cache/temp directories, generated LLM exports, bytecode, temporary files, environment variants, private-key/certificate containers, and common credentials/secrets JSON files. Canonical sources remain authoritative; repair stale context by correcting the source and regenerating it.

The lock records selected paths, included byte/file counts, and exclusion counts. `--check` verifies those fields and the complete source manifest. If either corpus export exists locally, it must match its current projection; absent exports do not make the source lock stale.
