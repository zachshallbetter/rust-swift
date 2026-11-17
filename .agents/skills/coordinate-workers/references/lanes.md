# Lane setup reference

Doctrine: `.agents/bootstrap/docs/MULTI_AGENT_COORDINATION.md`, "Lane coordination". This file holds the templates.

## Set up

1. Put the lanes on the board: add Lane and Blocked by fields to the work graph; give every item an Epic and a Lane.
2. Write one brief per lane (below). Keep each brief to its ownership, tickets, rules and state folder.
3. Create the state folder beside the worktrees and the lane folders:

   ```bash
   python3 .agents/skills/coordinate-workers/scripts/lane-watch.py ~/Projects/_worktrees/<repo>/.lanes --init core panel media
   ```

   The folder is untracked and outside every repository.
4. Pin one bulletin issue. The coordinator comments on it; lanes read its latest comments at each ticket start.
5. Start the monitor in the coordinator session, in the background:

   ```bash
   python3 .agents/skills/coordinate-workers/scripts/lane-watch.py ~/Projects/_worktrees/<repo>/.lanes
   ```

6. Start one session per lane. Record the owner's autonomy delegation, if any, with their words and the date, in the project's workflow document.

## Lane brief

An agent definition (for example `~/.claude/agents/<project>-<lane>.md`):

```markdown
---
name: <project>-<lane>
description: <Lane> lane engineer for <project>. Owns <areas>. Use for tickets <list>.
model: sonnet
---

## Your lane
- Owns: <paths, namespaces, epics>
- Order: <ticket sequence>
- Shared contracts: agree in the ticket, merge alone, then build.

## Every task
0. Read the latest comments on the pinned bulletin (<issue>).
1. Read <workflow doc> and <rules doc>; then only what the ticket needs.
2. Read inbox.jsonl; take the top Ready ticket in your lane that nothing blocks; set it In progress.
3. Work in ~/Projects/_worktrees/<repo>/<name>; one ticket, one branch, one PR, with tests.
4. Bump the version and changelog last, after rebasing.
5. Open the PR and set In review. Enable auto-merge only under a recorded owner delegation; otherwise the coordinator merges after checks.
6. An open decision blocks you: write or extend its memo and take other Ready work.

## Always
- Change only the files your lane owns.
- Run every build and test through the machine register, in the background. Never start a hosted build (preview, deploy, CI run) unless the ticket asks for one; the local gate is the check.
- Ask the coordinator, never the owner and never another project's session.
- Plain, declarative voice; no em dashes as separators; no AI attribution.
- Keep context small; stop and report when blocked.

## Lane state file
Your folder is <state>/<lane>/. Keep state.json current. Read inbox.jsonl at every ticket start and after every PR. Ask by adding a question and watching the inbox in the background.

## Finishing
On merge, remove the worktree and local branch in the same turn; then take the next Ready ticket.
```

## state.json

```json
{"lane": "core", "updated": "<ISO time>", "ticket": 95, "status": "working",
 "prs": [{"n": 152, "state": "open"}], "note": "", "inbox_seen": "c-3",
 "questions": [{"id": "core-1", "asked": "<ISO time>", "text": ""}]}
```

`status`: `working`, `review`, `blocked`, `idle`. Set `updated` on every write. A new `blocked` report needs a new `updated`.

## inbox.jsonl

```json
{"id": "c-4", "re": "core-1", "at": "<ISO time>", "text": ""}
```

`re` is the answered question id, or null.

## Decision memo

```markdown
# NNNN Short name
Status: proposed | accepted (date, "owner's words" or "coordinator decision") | rejected (date)

## Question
## Options
1. <option>: <cost>
2. <option>: <cost>
## Recommendation
## Unblocks
<tickets>
```

## Local test identity checklist

Implemented by the project, not shipped here. Required properties: one throwaway user with a random password in an untracked file; a fresh random session secret; static owner tokens dropped; loopback bind only; writes to a test store; refuses to start in production; documented in the project's verify procedure.
