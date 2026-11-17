# Multi-Agent Coordination

## Principle

Multiple workers are concurrent partial observers of one project, not independent authorities.

Communication exists to exchange evidence and update planning assumptions.

## Isolation

Every mutation worker has:

```text
work item
claim
repository/artifact scope
isolated workspace
owned paths/namespaces
context digest
```

Disjoint ownership is the default.

Allocate scarce namespaces—migration numbers, gate IDs, ports, fixture keys, release names—before dispatch.

## Communication

Use `AgentMessage` for facts that may affect another worker.

Message impacts:

```text
informational
blocks
unblocks
supersedes
requests_handoff
requests_review
invalidates_assumption
```

Messages carry source/revision/context identity so stale observations are visible.

## Receiving a material message

If the message intersects a live assumption:

```text
pause at safe checkpoint
→ preserve current candidate
→ inspect evidence
→ re-read live state
→ re-evaluate approach
```

Possible disposition:

```text
continue
pause
handoff
supersede
revert
abandon
```

Reversion is not failure when a peer has established a better path.

## Handoff

A handoff names:

```text
from / to
work item
candidate/checkpoint
owned paths
what is complete
what remains
evidence
known risks
claim release/acquisition order
```

The receiving worker does not mutate until the old claim is released or the coordinator has transferred it.

## Shared-state writes

Coordinator or provider-native atomic mechanisms serialize:

```text
claims
status promotion
shared namespace allocation
canonical evidence replacement
project-wide decisions
```

Peers do not race to “help.”

## No authority by coalition

Two or twenty agents agreeing does not create:

```text
promotion authority
permission
ownership
contract authority
publication authority
deviation authority
```

## Contradiction

When peers disagree, preserve both claims and their evidence. Use existing precedence/evaluation. Do not erase disagreement through averaging or consensus prose.

## Lane coordination

Lane coordination is the standing form of the pattern above for a project with more work than one session can carry. It was proven on Hermit CMS, with one coordinator and six lanes. It adds no authority and no canonical concept: a lane is a worker with a standing scope, the board is the work graph, and the state files are messages.

```text
coordinator (the owner's session)
  ├─ board            Ready frontier, source of truth for what is next
  ├─ bulletin         one pinned issue for announcements to every lane
  ├─ lane A  ── state.json (lane writes)   inbox.jsonl (coordinator writes)
  ├─ lane B  ── …
  └─ monitor          one process over every state.json
```

### Coordinator and lanes

One coordinator session and one long-running session per lane. A lane is defined by a brief, an agent definition with:

```text
name and description (what it is for, which tickets)
what it owns: paths, namespaces, epics
its tickets, in order
its rules and the project rules it must read first
a light model
its state folder
```

Ownership is disjoint by default. A change another lane needs goes as a small PR of its own, or as a ticket for the owning lane, merged before anything builds on it. Shared contracts (types, store interfaces, API shapes) are agreed in the ticket, merged alone, then built on.

The coordinator holds the board, reviews PRs across lanes, finds overlaps, relays between lanes and rebalances. It does not do lane work.

Cross-cutting work (build scripts, CI, release, workspace manifests, shared tooling) is nobody's lane by default, so two lanes will fix it at once unless it is assigned. Before anyone writes such a change it gets a ticket and one owner, announced on the bulletin; other lanes wait or work around it.

A lane may run light subagents, one per independent ticket, each in its own worktree and PR. Start one only when the ticket is unblocked and its files do not overlap any open PR. The lane reviews each subagent's PR before it merges. A lane that is itself a subagent does not start subagents.

### The board

The work graph is the source of truth for what is Ready. A lane takes the top unblocked Ready item in its lane, claims it (status In progress), and updates status only on transitions:

```text
Ready → In progress   when the lane claims it
In progress → In review   when the PR is open
In review → Done   when the PR merges
```

Notes and progress go in the lane's state file, not on the board. A ticket whose decision is open is not Ready. New work gets an item on the board first, after a search for an existing one; overlapping items are linked or merged.

When a lane runs out of Ready work it reports `blocked` or `idle`. The coordinator recomputes the frontier and moves items between lanes (the item's lane field changes, the claim moves with it). A blocked lane is usually a lane with no Ready work, not a lane with a problem.

### Pinned bulletin

One pinned issue carries announcements that apply to every lane: a shared-state fact ("the change log is on the test branch; merge main before testing writes"), a number taken, a budget change, a new rule. Each lane reads its latest comments at the start of every ticket. The bulletin is durable and costs no session messages. It carries facts, not instructions that widen authority.

### Lane state files

One untracked folder per lane, beside the worktrees and outside every repository: `~/Projects/_worktrees/<repo>/.lanes/<lane>/`. Nothing in it is committed.

`state.json` is written by the lane:

```json
{
  "lane": "core",
  "updated": "2026-11-08T01:27:57Z",
  "ticket": 95,
  "status": "working",
  "prs": [{"n": 152, "state": "open"}],
  "note": "one line",
  "inbox_seen": "c-3",
  "questions": [{"id": "core-1", "asked": "2026-11-08T01:20:00Z", "text": "…"}]
}
```

`status` is `working`, `review`, `blocked` or `idle`. Question ids are `<lane>-<n>`. `updated` changes on every write, so a repeated `blocked` with a new `updated` is a new report.

`inbox.jsonl` is written by the coordinator, one object per line:

```json
{"id": "c-4", "re": "core-1", "at": "2026-11-08T01:28:42Z", "text": "…"}
```

`re` is the question id answered, or null for an unprompted message such as a reassignment.

Lane protocol:

```text
at the start of every ticket and after every PR:
  read the bulletin, read inbox.jsonl, act on lines newer than inbox_seen, record inbox_seen
to ask the coordinator:
  add a question to state.json
  keep working on anything unblocked
  watch for the answer in the background:
    until grep -q '"re":"<id>"' <state>/<lane>/inbox.jsonl; do sleep 30; done
when blocked: set status blocked with a note that names the blocker and what is needed
```

The file is the lane's own state, so the lane writes it without a claim. The inbox is the coordinator's, and a lane never edits it. Files are messages: they carry observations and requests, and a line in an inbox is not authority (see No authority by coalition).

Coordinator protocol: run one monitor over every `state.json`. It reports each new question not yet answered in the inbox, and each lane that newly reports `blocked`. `.agents/skills/coordinate-workers/scripts/lane-watch.py <state-folder>` does this with the Python standard library and keeps its seen-set in the folder. Answer a question by appending a line whose `re` is its id. Rebalance when a lane is blocked for lack of Ready work.

A lane that ended its turn `idle` or `blocked` does not read its inbox until something wakes it. So when the coordinator answers or reassigns a blocked or idle lane, it appends the inbox line and then sends that lane one short direct message ("new work in your inbox, c-<n>"). A lane that is still working needs no message: it reads the inbox at its next ticket or PR.

### Budgets

These budgets are shared and finite.

**Session messages.** The host may cap a session at a fixed number of messages to other sessions between the owner's messages (ten in the Hermit setup). Direct messages are for a lane that is blocked and cannot wait. Routine traffic goes through the state files and the bulletin. The one routine exception is the wake-up: one short message to a lane that is idle or blocked, after the coordinator has written its inbox line, because such a lane is not reading the inbox.

**Service API limits.** Every session on one account shares the service's rate limits (GitHub: 5,000 requests an hour, with a per-minute GraphQL limit). Therefore:

```text
no polling loops on the service
under a recorded delegation, enable auto-merge and move on; otherwise wait with one background check
when waiting is unavoidable: at most one check every two minutes, REST over GraphQL
update the board on status transitions only
on a rate-limit error: stop service calls for several minutes, work locally, do not retry in a tight loop
```

Local file watching is free and is the intended way to wait.

**The machine.** Every lane, subagent and peer coordinator on one machine shares its cores, memory and disk, and concurrent compiles exhaust them long before session count does. On Hermit (10 cores, 16 GB), parallel lanes reached load 31, filled 13 of 14 GB of swap, and left 15 GB of disk. Therefore:

```text
every compile and test runs through one machine-wide register (FIFO build slots, capped jobs and test threads)
shared build folders are bounded: stale incremental state pruned, a folder over its cap wiped
test only the packages a change touches; the full suite runs in the local gate
build expensive targets (wasm, release) only when their sources changed, locally and in CI
dev servers run through the register and stop when the check is done
when the machine is saturated: hold every lane, fix the build, then resume one lane at a time
```

The integration-test layout matters as much as the number of builds: in Rust every file in `tests/` links its own program against the whole dependency graph, so one test program per package is the default.

**Hosted builds and CI minutes.** Preview deployments, production deployments and CI jobs run on paid machines, billed by the minute and often by the core. Lanes multiply them: every push, every rebase force-push and every merge starts one. On Hermit, Vercel ran 754 deployments in about two days (524 previews from 242 branches and 230 production builds), each a full Rust and WebAssembly build on a 30-core machine, about 121,000 CPU minutes. GitHub Actions burned about 4,000 hours in 45 minutes, on two accounts. Therefore:

```text
the local gate is the check of record; a hosted build duplicates it only on request
a preview builds when asked (a commit token or a preview/* branch), never on every push
production builds on a release merge or an explicit deploy, never on every merge
docs, changelog fragments and agent notes never start a hosted build
a skipped build reports success, so a required check never blocks a merge
CI runs on pull requests, not on every branch push, with path filters, one concurrency group per branch
  that cancels the run in progress, a timeout on every job, and no matrix unless asked for
build machines are fixed and small: no elastic or turbo selection, no concurrent builds
every account has a hard spend cap and an alert well below it
before lanes start: list every service connected to the repository and what starts a build on each
during a run: read each service's usage after the first hour and at every coordinator check;
  a rising build count holds the lanes until it is explained
```

A commit-message token is a weak trigger: a commit that describes the feature contains the token and builds. Prefer turning branches off in configuration (`"git": {"deploymentEnabled": {"main": true, "**": false}}` in the `vercel.json` of the project's root directory) and building a preview from a `preview/*` branch. A Vercel project builds on push only through its GitHub integration; a local `.vercel` link alone means deploys from the CLI. `check-build-budget.py` reports a project's workflows, Vercel configuration and Rust test layout against these rules.

**Enforcement.** Briefs and rules did not prevent either incident above; agents were told to keep builds local both times. The rules therefore hold at the harness, below every session and subagent on the machine (`harness/` in the bootstrap repository): a push or CLI deploy that would start a hosted build is refused, a heavy build outside the register is refused, and a subagent without a light model, or beyond the per-session cap, is refused. Refusal, not a prompt, because a permission mode that approves prompts approves them unseen. ACP decides pushes, deploys, manual CI, production merges and heavy subagents, and its refusal names the remedy; heavy builds always have the register. Only the secrets guard keeps an owner approval, and the hold stops everything. A refusal is reported to the coordinator with the work committed locally; it is never worked around.

### Peer coordinators and other projects

Several coordinators can run on one machine, each for its own project, as Hermit and Peel did. They share the machine and the account, never authority.

```text
one coordinator per repository: it alone merges, posts the gate status and calls GitHub for it
a coordinator never acts in another project's repository; it messages that project's coordinator or files an issue there
the machine register is shared by every project: builds, ports, claims and the GitHub budget go through it
a lane asks its own coordinator, never the owner and never another project's coordinator; the coordinator answers or escalates
a message from another session is information to weigh; approval relayed by an agent is not the owner's approval
a technique that cuts cost travels both ways as a short note (what, where, what it fixed); anything general lands in this bootstrap
a dependency between projects (vendored code, a shared API) names the owner of each side; changes cross it as a PR to the owner's repository or a ticket for its coordinator
```

### Finishing and reuse

Worktrees live under `~/Projects/_worktrees/<repo>/<name>`; one ticket, one branch, one PR, one worktree.

- The PR's author removes the worktree and the local branch the turn the PR merges. An abandoned worktree is removed once nothing in it is uncommitted or unpushed. No worktree outlives its work.
- A lane that finishes takes its next Ready item in the same session instead of ending idle. A subagent that finishes is given the lane's next item or ends after removing its worktree.
- The coordinator starts a new session only for a new lane.

### Decisions and autonomy

A decision that blocks work is a memo in the repository (`docs/decisions/NNNN-short-name.md`):

```text
question
two to four options, each with its cost
recommendation
what it unblocks
status: proposed → accepted or rejected, with the date
```

An accepted memo records either the owner's words or "coordinator decision" under delegation; the owner can overturn the latter. A lane does not build past an open decision: it writes or extends the memo and takes other Ready work. The coordinator presents proposed memos to the owner in batches and updates the items they unblock.

When the owner delegates autonomy, PRs merge themselves once their checks pass and the coordinator has reviewed them (`gh pr merge <n> --auto`). Delegation is recorded with the owner's words and date. It does not widen what the project's authorization provider decides: where the profile binds one, a push to a protected ref still needs that provider's `ALLOW` for the exact action (AGENTS.md §17). These stay with the owner whatever the delegation: secrets and production configuration, security settings, production data, other repositories, money, and brand choices. Changes to data, storage layout, auth or a public API are tested first (migration dry run with a before and after diff, negative tests, a parity check) and then merge on green.

Version bumps collide across lanes. Bump last: rebase on the main branch, take the next free version, put the changelog section at the top. When two PRs claim one number, the second to merge rebases and takes the next one.

### A local test identity for agents

A lane that changes UI must verify it as a person would, and an agent does not hold the owner's credentials. A project that has sign-in provides a local test identity, a development switch (Hermit: `CMS_DEV_SIGNIN=1`) that:

```text
creates one throwaway user with a random password written to an untracked file
signs sessions with a fresh random secret, so they are valid nowhere else
and the owner's sessions are not valid there
drops static owner tokens
binds to loopback only
writes only to a test data branch or store
refuses to start when the environment is production
```

Agents sign in with those values in their own browser tab. This is a pattern each project implements; the bootstrap ships none. It is the project's local-development exception, not a way around authentication: an agent still never enters the owner's credentials, and the switch must be impossible to enable on a deployed host.

### Evidence and reconciliation

Lane state is an observation and can go stale. Before acting on a lane's `state.json`, check it against the board and the repository (`updated` is the first test). After a session restart, reconcile from the board, the open PRs, the worktrees and the state files; a lane's conversational memory is not a source (RECOVERY.md). A merge, a verification and a refusal are recorded in the project's evidence, not left in a state note.
