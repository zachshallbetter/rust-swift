---
name: operate-frontier
description: Use to autonomously execute Ready work across a project. Handles selection, claim, isolation, bounded execution, localized blockers, release and frontier reselection.
---

# Operate Frontier

## Requirements

Use in a project with a live work graph, claim and isolation mechanisms, acceptance/evidence rules, and a bound authorization path for governed effects. If the live work graph or ACP is unavailable, the local E0/E1 path below remains available; it never grants remote authority.

1. Reconcile using the active project's profile and provider bindings.
2. When the live graph is available, select an unclaimed Ready item with satisfied dependencies that fits authority and budget. Acquire its claim and isolated workspace through the configured providers, then re-read live state to confirm ownership.
3. When the board cannot be read or ACP is unreachable, use only a Ready item in `.agents/records/local-work-items.jsonl`. Run `python3 .agents/bootstrap/scripts/validate-local-work-items.py`, check its intent, dependencies, acceptance, exclusions, evidence requirements, budget, and stop conditions, and work only within its declared local scope. E0 permits observation and analysis; E1 permits reversible local edits and commits. Do not use this path to continue a refused or unavailable governed action, or to push, merge, deploy, alter policy/credentials, or perform recovery.
4. Compile work context using the active project's context policy and current source lock.
5. Execute the smallest correct change within the selected item's authority and scope.
6. Before any governed effect, use `authorize-protected-effect` and the provider bound by the active project's profile; proceed only within the exact authorization returned.
7. On a local blocker (including refused or unavailable authorization): record it, release or park any live claim, notify affected peers with only task-relevant details, then reselect unrelated Ready work.
8. On a global stop (including `QUARANTINE`/`LOCKED` for the resource in scope): prepare the smallest decision packet and stop work in that scope.
9. On candidate completion: submit for evaluation/qualification required by the issue and record the required evidence. Reconcile completed local items into the live board after service returns; that synchronization is not authorization.
10. Record disposition using the project's work/evidence contracts, release any live claim through its provider, and reselect if lawful work remains.
