# Work Graph

## States

```text
Backlog
Ready
In progress
In review
Done
Verified
```

Provider labels may differ; meanings must not.

## Ready predicate

```text
scope complete
∧ exclusions complete
∧ dependencies satisfied
∧ acceptance defined
∧ evidence requirements defined
∧ permissions/effect class valid
∧ target workspace resolvable
∧ budget valid
∧ stop conditions present
```

Evaluator identity is required when independent evaluation is part of acceptance.

## Work item

```ts
interface WorkItem {
  id: string;
  intentRef: string;
  parentRef?: string;

  objectiveOrHypothesis: string;
  scope: string[];
  exclusions: string[];
  dependencies: string[];

  acceptance: string[];
  evidenceRequirements: string[];
  evaluatorRef?: string;

  permissions: string[];
  effectClass: "E0" | "E1" | "E2" | "E3" | "E4";

  target: {
    repositoryOrArtifact: string;
    authorizedPaths?: string[];
  };

  budget: {
    retries?: number;
    money?: number;
    humanAttentionMinutes?: number;
  };

  stopConditions: string[];
}
```

Domain profiles may attach candidate/asset/etc. references without modifying the base work-state meaning.

## Claim semantics

```text
read live item
→ acquire claim
→ reread
→ execute only if ownership still holds
```

One active mutation claim per worker. Controlled batching must be explicit.

## Blocker semantics

A blocker records:

```text
type
scope
evidence
recovery condition
affected dependencies
whether unrelated work may proceed
```

Blocked state does not erase the item.

## Local work while ACP or the board is unavailable

`records/local-work-items.jsonl` is the durable queue for independently scoped
local work when a GitHub board cannot yet be bound or ACP is unreachable. Each
line is one complete work item using the fields above plus `state`. Validate it
with `python3 scripts/validate-local-work-items.py` before acting on it.

The local ledger accepts active E0/E1 items and the local permission vocabulary
`repository.read`, `source.inspect`, `local.analysis`, `local.test`, `local.edit`,
and `local.commit`. A Ready local item may authorize only those local actions on
its declared paths. It does not authorize GitHub writes, protected pushes,
merges, deployments, policy or credential changes, or recovery. Those effects
still require an exact live ACP `ALLOW`. Reconcile local items into the project
board after service returns; board synchronization is not itself authorization.
Completed E2+ rows are accepted only as historical evidence with completion
evidence; they never authorize a current effect.

## Frontier

The coordinator selects from all currently Ready, unclaimed, dependency-satisfied items within authority and budget.

A blocked item removes only itself and its dependents from the frontier unless the blocker is global.
