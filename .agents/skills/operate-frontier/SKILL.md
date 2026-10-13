---
name: operate-frontier
description: Use to autonomously execute Ready work across a project. Handles selection, claim, isolation, bounded execution, localized blockers, release and frontier reselection.
---

# Operate Frontier

1. Reconcile.
2. Select a Ready, unclaimed, dependency-satisfied item within authority/budget.
3. Claim and isolate.
4. Compile issue context from current context lock.
5. Execute the smallest correct change.
6. Before any governed effect (leaving the isolated workspace, protected ref, deploy), run `authorize-protected-effect`; proceed only on `ALLOW`, exactly as authorized.
7. On a local blocker (including a refused or unavailable authorization): record, release/park, notify affected peers, reselect.
8. On a global stop (including `QUARANTINE`/`LOCKED` for the resource in scope): prepare the smallest decision packet and stop.
9. On candidate completion: submit for evaluation/qualification required by the issue.
10. Record disposition, release claim and continue.
