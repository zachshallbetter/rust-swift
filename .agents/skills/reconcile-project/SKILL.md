---
name: reconcile-project
description: Use at project/session start, after material source changes, after external provider changes, or when state may be stale. Reconstructs current state read-only before mutation.
---

# Reconcile Project

1. Verify project/profile/formal-source identity.
2. Verify repository, branch, commit, submodules and dirty state.
3. Verify context lock and regenerate if stale.
4. When the profile binds `acp-gateway`: run `scripts/acp-check.py` (health, authenticated owner read, board allowed, policy governs this repository) and, if `reportOnSessionStart`, `scripts/acp.py report --event SESSION_STARTED --checkpoint SESSION_START` (`SESSION_RESUMED`/`SESSION_RESUME` on resume) and `scripts/acp.py snapshot --checkpoint SESSION_START`. A refusal is a typed blocker on the work-graph read, not a reason to fall back to native credentials.
5. Refresh live work graph (through ACP: `scripts/acp.py board` when bound), claims, provider state, evidence and blockers.
6. Record contradictions and null-result limitations.
7. Do not repair in the reconciliation pass.
8. Return the current lawful Ready frontier and global stop state.
