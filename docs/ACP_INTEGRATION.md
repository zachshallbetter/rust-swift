# ACP Integration

**Status:** Canonical provider binding  
**Provider:** `acp-gateway` — the Agent Control Plane authorization gateway  
**Binds:** `docs/AUTHORITY_MODEL.md`, `docs/EXECUTION_PROVIDER_CONTRACT.md`, `AGENTS.md` §9 and §17

ACP is the external authority that decides whether a protected effect may be caused. It is bound as the project's `protection` provider and as the `authorization` instrument for every effect class at or above the level declared in `PROJECT_PROFILE.json`.

ACP does not replace project intent, the authority model, pinned resources, or the work graph. It is the instrument that **authorizes** a protected effect; every other record attached to that effect (evidence, receipt, telemetry report, claim, peer message, conversation instruction) merely observes, remembers, or pressures. This is the instrument stratification rule applied to one provider.

## What ACP provides

```text
board read          GET  /internal/project-context?owner=&project=   work-graph read without holding GitHub credentials
board discovery     GET  /internal/installations, /internal/projects?owner=
governance notice   GET  /notice?owner=&name=                          unauthenticated: is this resource governed
authorization       POST /internal/authorize                          decision for one action against one repository
capability check    POST /internal/verify-capability                  check an ALLOW capability at the protected boundary
recovery report     POST /internal/recovery/complete                  outcome of one authorized recovery transition
telemetry           POST /internal/report                             checkpoints, control-plane changes, anomalies; never authorizes
integrity snapshot  POST /internal/integrity/snapshot                 control-artifact hashes against the policy's protected manifest
liveness            GET  /health                                      unauthenticated
readiness           GET  /ready                                       unauthenticated; requires shared PostgreSQL readiness
```

ACP is pull-only and never writes the work graph. Claims, field updates and item creation use the worker's own scoped credentials through the project's `workGraph` adapter.

`scripts/acp.py` is the project's client for every call above (standard library only). Agents use it rather than hand-built requests, because it applies the rules in this document that a script can apply without judgement: fail-closed synthesis, `NOT_GOVERNED` refusal, exact-action checking, and evidence recording without secrets.

```bash
python3 scripts/acp.py authorize --action git.protected_ref.update \
    --ref refs/heads/main --old-sha "$OLD" --new-sha "$NEW" --work-item 42 --record > decision.json
python3 scripts/acp.py verify-capability --decision decision.json     # at the protected boundary
python3 scripts/acp.py report --event SESSION_STARTED --checkpoint SESSION_START
python3 scripts/acp.py snapshot --checkpoint PRE_PUSH
python3 scripts/acp.py notice                                         # governed? (exit 5 if not)
python3 scripts/acp.py board                                          # the bound board, through ACP
python3 scripts/acp.py policy                                         # candidate policy for registration
```

The bootstrap client can also operate on another checkout without copying its
script. Invoke it with `--project-root /path/to/repository` before the command;
the path must be that Git worktree's root. It reads that repository's profile
and ignored `.env.local`, and reports its actual branch and commit:

```bash
python3 /path/to/formal-project-bootstrap/scripts/acp.py \
  --project-root "$PWD" authorize --action production.deploy --environment production
```

`authorize` exits `0` only for `ALLOW`; every other decision has its own non-zero code (`11` DENY … `17` VERIFY_RECOVERY, `2` for missing local configuration), so a shell step cannot proceed on a refusal by accident. The other commands exit `0` accepted, `2` configuration, `3` refused, `4` unreachable, `5` not governed.

The client validates the requested environment locally. Ordinary authorization requests send the action's exact ref and SHA context; the `environment` field is sent with a shared owner-override ID or for `database.migrate`, whose signed scope requires it.

`database.migrate` requires `--context-json` with exactly the non-secret `source_database` and `target_database` identifiers, plus `--work-item`, `--environment`, `--ref`, and `--new-sha`. The gateway verifies that the checked-out commit and independently read GitHub ref match that full revision, and binds all six values into the decision and capability. A migration request never implies that writes are paused or that schema/data preflight passed; those remain separate evidence and operational conditions.

## Binding in the project

`PROJECT_PROFILE.json` declares:

```json
"authorizationProvider": {
  "name": "acp-gateway",
  "gatewayEndpointSet": "production",
  "boardEnvFile": ".agents/board.env",
  "authorizedEffectClassesMinimum": "E2",
  "failClosed": true,
  "reportOnSessionStart": true,
  "reportBeforeProtectedEffect": true,
  "actionMap": "contracts/acp-protected-effects.yaml",
  "checkScript": "scripts/acp-check.py",
  "clientScript": "scripts/acp.py"
}
```

`bindings/PROJECT_BINDINGS.yaml` sets `provider_bindings.protection: acp-gateway` and records, per protected effect, that the **holder** is `acp-gateway` (decision) with the project's human authority named as the **deviation** and **recovery** holder.

`contracts/acp-protected-effects.yaml` maps project effect classes and operations to ACP action names. The map is a projection of the authority model onto ACP's vocabulary; it does not add authority.

Machine-local endpoint registry:

```text
macOS: ~/Library/Application Support/formal-project-bootstrap/acp-endpoints.json
Linux: $XDG_CONFIG_HOME/formal-project-bootstrap/acp-endpoints.json
ACP_ENDPOINT_REGISTRY           optional absolute path override for the machine-local registry
Keychain service                 ACP Gateway Production
Keychain accounts                one distinct ACP_GATEWAY_TOKEN* account per endpoint
.agents/board.env                committed board/repository identity; no gateway URL or bearer
```

Initialize the machine registry once. It stores endpoint URLs, Keychain account names, and the expected shared state authority ID; bearer values remain in Keychain. Set the same `ACP_SHARED_STATE_ID` on every gateway instance. The endpoint order is significant:

```bash
python3 scripts/acp.py endpoints configure --set-name production \
  --shared-state-id acp-production-shared \
  --endpoint vercel https://acp-gateway.vercel.app ACP_GATEWAY_TOKEN \
  --endpoint titan https://titan.your-tailnet.ts.net ACP_GATEWAY_TOKEN_TITAN
python3 scripts/acp.py endpoints list
```

The currently verified production set contains only `https://acp-gateway.vercel.app`;
it is a single endpoint, not failover. Do not add a secondary endpoint until it
has HTTPS, an independently provisioned credential, a ready PostgreSQL-backed
response with the same `stateAuthorityId`, and verified parity for shared state
and the audit chain. Keep an unavailable host out of the set so projects select
only endpoints the client has actually established as usable.

Use a separate set for an explicitly selected loopback instance:

```bash
python3 scripts/acp.py endpoints configure --set-name local --local-only \
  --shared-state-id acp-production-shared \
  --endpoint loopback http://127.0.0.1:8080 ACP_GATEWAY_TOKEN_LOCAL
```

Each project profile selects one set by name. The client probes each selected endpoint's unauthenticated `/ready` and accepts only HTTP 200 with `status: ready` and `storage: postgres`; each probe has a two-second cap. It never mixes endpoint sets or silently adds loopback. Each endpoint uses its own Keychain account. A multi-endpoint set requires one explicit `stateAuthorityId`; each `/ready` response must report the matching `ACP_SHARED_STATE_ID`, or the client stops without routing a request. This identifier catches configuration mismatches; release verification must still prove both instances use the same transactional database by comparing state and audit-chain results across instances. A read-only GET can fail over after a transport failure or a typed transient `UPSTREAM_FAILURE`, `TEMPORARY`, or `SERVICE_UNAVAILABLE` response. A 401, a valid ACP decision, an integrity/control-state error, or an unclassified server error stops routing. For POST routes, the client sends one `Idempotency-Key` and retries across ready endpoints only when `/ready` advertises idempotency version 1 for that exact route and matching state authority. The gateway advertises this only with PostgreSQL available and response-encryption configuration valid; its replay records and effects are shared transactionally. Unsupported routes are sent once and never retried. A valid ACP decision or any untyped error is returned without failover. Requests/responses are capped at 8 MiB and redirects are refused so bearer credentials cannot be forwarded to another host. Remote endpoint URLs require HTTPS; HTTP is allowed only for loopback endpoints explicitly marked local-only.

Legacy profiles with one URL environment binding normalize to one endpoint at the client boundary. New profiles use `gatewayEndpointSet`; multi-endpoint project-local arrays are rejected so endpoint ordering and credentials have one machine-local source of truth.

Readiness failures identify the endpoint name, `/ready` path, HTTP status when
received, response shape, and elapsed milliseconds for eligible failed probes.
Transport failures are distinct from an HTTP refusal. Diagnostics name the ACP
operator as the resolver and preserve the local E0/E1 path; authentication,
control-state, authority-identity, and unclassified server failures explicitly
stop failover. They never print upstream response contents, bearer values, or
database connection strings. A failed client probe alone does not establish a
gateway or database outage; correlate its time with service request logs.

`CONTEXT_SOURCES.json` excludes `.env.local`. `scripts/acp-check.py` uses the same ordered endpoint selection without printing credentials. Set `ACP_KEYCHAIN_LOOKUP=0` to disable the macOS fallback. `ACP_POLICY_ADMIN_TOKEN` is never loaded from Keychain implicitly and must be supplied explicitly for administrative actions; projects with different endpoint admin credentials may bind `adminTokenEnv` per endpoint.

Optional identity for audit (never authority): `ACP_AGENT_ID`, `ACP_ACTOR_ID`, `ACP_SESSION_ID`. `ACP_REMOTE_URL` is the canonical repository identity for bootstrap setup; if it or `origin` is present, conflicting `ACP_REPOSITORY_OWNER`/`ACP_REPOSITORY_NAME` overrides fail closed. Owner/name overrides are used only when no remote is configured.

## Governed means a policy names the repository

Binding the provider in the profile does not make ACP govern the repository. ACP governs a repository only when a policy registered **in the gateway** names it. Until then:

| Where the repository is | What ACP answers for a protected action | What `scripts/acp.py` returns |
|---|---|---|
| inside the gateway's `ACP_POLICY_SCOPE`, no policy | `DENY` / `POLICY_NOT_FOUND` | the same |
| outside that scope, no policy | `ALLOW` / `OUT_OF_POLICY_SCOPE`, `policy_effect: NOT_GOVERNED` | `DENY` / `ACP_NOT_GOVERNING` (synthetic) while `failClosed` |
| a policy names it | a real decision | the same |

An `ALLOW` with `policy_effect: NOT_GOVERNED` is ACP declining jurisdiction, not granting authority; it carries no `authorized_action` and no capability. A fail-closed project treats it as a setup blocker. `scripts/acp-check.py` and `acp.py notice` exit `5` for the same reason, distinct from a refusal (`--allow-ungoverned` downgrades it to a warning during onboarding).

## Effect classes and ACP actions

| Effect class | Governed by ACP | Typical ACP actions |
|---|---|---|
| E0 observational | no — self-reported local actions | `repository.read`, `source.inspect`, `local.analysis`, `local.test` (informational) |
| E1 reversible internal mutation | no — isolated branch/worktree work | — |
| E2 reversible operational mutation | yes when it leaves the isolated workspace | `git.remote.update` (non-protected ref) |
| E3 consequential external commitment | yes | `git.protected_ref.update`, `production.deploy` |
| E4 durable/irreversible | yes, and requires a server-side recovery authorization | `git.history.rewrite`, `git.default_branch.change`, `repository.transfer`, `credential.modify`, `policy.modify` |

The project may raise the minimum governed class (for example to `E1` for a project whose isolated workspaces are shared) but must not lower it below what the authority model declares as protected.

## Decisions and the required disposition

| ACP decision | Code | Agent disposition |
|---|---|---|
| `ALLOW` | A0 | Perform **exactly** `authorized_action` (same action, ref and SHA pair) within the capability TTL. Nothing else. |
| `DENY` | A1 | Do not proceed. Record `AUTHORITY_MISSING`; release or park the claim; continue unrelated Ready work. |
| `AUTH_REQUIRED` | A2 | Complete the ACP-designated flow named in `auth`, then retry once. |
| `REVERIFY_REQUIRED` | A3 | Do not proceed. Repository trust dropped; record a provider blocker. Do not attempt repair. |
| `QUARANTINE` | A4 | Do not proceed and do not attempt repair. Preserve candidate and evidence; escalate as a global stop for this resource. |
| `LOCKED` | A5 | Controlled operations suspended. Global stop for this resource until administrative recovery. |
| `RECOVERY_AUTHORIZED` | A6 | Perform only the one described recovery transition, once, then report its outcome to `/internal/recovery/complete`. |
| `VERIFY_RECOVERY` | A7 | A recovery was executed and is not yet proven. Do not proceed with protected effects. |

`repository_trust` (`VERIFIED` / `SELF_REPORTED` / `UNVERIFIED` / `QUARANTINED`) is evidence about the repository, not a decision. `SELF_REPORTED` is never sufficient for a protected action.

An `ALLOW` or `RECOVERY_AUTHORIZED` carries `authorized_action` (action, ref, `old_sha`, `new_sha`) and, when the gateway holds a signing key, a short-lived `capability_token`. The token — not the `capability` object, which only describes it — is what `/internal/verify-capability` and `/internal/recovery/complete` accept. It is passed to the next step and never written to `records/`. A gateway without a signing key issues no token; nothing at the boundary can then verify the decision, and that gap is recorded rather than papered over.

Decisions the client synthesizes locally are marked `synthetic: true` and are always `DENY`:

| Synthetic reason | Cause |
|---|---|
| `GATEWAY_UNAVAILABLE` | unreachable, non-200, not JSON, missing required fields, or a decision outside A0–A7 |
| `ACP_NOT_GOVERNING` | `ALLOW` with `policy_effect: NOT_GOVERNED` for a governed action under `failClosed` |
| `AUTHORIZED_ACTION_MISMATCH` | `ALLOW` whose `authorized_action` differs from the requested action, ref or SHA pair |

Every decision carries a stable `reason`. Record `decision`, `reason`, `request_id`, `policy_id`, `policy_version` and `repository_trust` in the work disposition and in `records/evidence.jsonl`.

## Owner override for local experimentation

An owner may temporarily suppress the bootstrap client's ACP checks for one
project and either the current branch or the whole project, in the `local` or
`preview` environment. This is an explicit local opt-out, not an ACP decision:
it is stored in the worktree's private Git metadata, expires after one hour by
default (maximum eight hours), and is also recorded in
`records/evidence.jsonl`.

```bash
python3 scripts/acp.py override grant --scope branch --environment local \
  --reason "Try a local implementation" --authorized-by <owner>
python3 scripts/acp.py override list
python3 scripts/acp.py override revoke <override-id> --authorized-by <owner>
```

Use `--scope project` to cover every branch and `--expires-in <seconds>` to
choose a shorter or longer lifetime within the eight-hour limit. The check and
client report/snapshot/notice/board commands print that the override is active
and record its use. A branch change, environment mismatch, expiration, or
revocation ends its effect. The override is local to this worktree and is not
copied to clones or CI.

Local overrides cannot authorize GitHub pushes, database migrations, policy or
credential changes, or deployments. Those effects target a remote environment
even when requested from a local checkout and continue to require an ACP
decision. A remote preview, staging, or production exception requires a signed
grant in shared ACP state and the separate admin credential:

```bash
python3 scripts/acp.py override shared-grant --scope branch \
  --environment preview --action git.protected_ref.update \
  --reason "Authorize this branch preview experiment" --authorized-by <owner>
python3 scripts/acp.py authorize --action git.protected_ref.update \
  --environment preview --ref refs/heads/<branch> --old-sha <old> --new-sha <new> \
  --owner-override-id <authorization-id>
python3 scripts/acp.py override shared-revoke <authorization-id> \
  --reason "Experiment complete" --authorized-by <owner>
```

Shared grants are limited to a repository, exact branch or project-wide scope,
environment, selected protected actions, expiry (one hour by default; eight
hours maximum), and a bounded use count. ACP records grant, use, and revocation
in its shared audit chain; every use produces a separate short-lived,
single-use capability. Grants do not override containment locks, ACP's own
policy/credential actions, or recovery actions. The gateway must be released
with the shared-grant API and database migration before this path is available;
the active deployment is not yet updated.

## Fail-closed rule

If ACP cannot be reached, returns malformed output, or a valid decision cannot be obtained, the protected effect is **not authorized**. Treat it as `DENY` with reason `GATEWAY_UNAVAILABLE`, classify the provider failure once, and do not poll. Reads (`E0`) and isolated work (`E1`) continue.

Board-read refusals (`BOARD_READ_CONTAINED`, `PROJECT_NOT_ALLOWED`, `OWNER_NOT_INSTALLED`, `PROJECTS_PERMISSION_MISSING`, `APP_NOT_CONFIGURED`, `UPSTREAM_FAILURE`) are typed blockers on the work-graph read. Surface the gateway's `error`, `detail` and `remedy` verbatim. A `BOARD_READ_CONTAINED` refusal is a containment signal, not a credential problem.

## Reporting is not authorization

A report is telemetry; it grants nothing and it must not be used as a substitute for a decision. Do not withhold reports because a decision was refused. The gateway accepts only these typed events on `POST /internal/report`; anything else is a `400`:

| When | Event (`acp.py report --event`) | Evidence |
|---|---|---|
| session start / resume | `SESSION_STARTED` / `SESSION_RESUMED` | `--checkpoint SESSION_START` / `SESSION_RESUME` |
| before commit, push, merge, deploy | `PROTECTED_EFFECT_PENDING` | `--checkpoint PRE_PUSH` (etc.) `--action <acp action>` |
| remote changed | `REMOTE_CHANGED` | `--detail` |
| branch protection changed | `BRANCH_PROTECTION_CHANGED` | `--detail` |
| policy or notice digest changed | `POLICY_DIGEST_CHANGED` / `NOTICE_DIGEST_CHANGED` | `--detail` |
| credentials rotated | `CREDENTIALS_ROTATED` | `--detail` (never the credential) |
| an anomaly was observed | `LOCAL_POLICY_MODIFIED`, `REMOTE_MISMATCH`, `ALTERNATE_AGENT_OBSERVED`, … | as observed |

Checkpoint and control-plane-change events are `INFO`/`NOTICE`: they never open an incident and never move authorization state. Anomaly events are graded by ACP, not by the reporter, and tampering reported while the resource is quarantined escalates it to `LOCKED`.

At the same checkpoints, `acp.py snapshot --checkpoint <C>` sends the hashes of the project's control artifacts (`AGENTS.md`, `PROJECT_PROFILE.json`, the bindings, this document, the action map, the authorize skill, `.agents/board.env`) to `POST /internal/integrity/snapshot`, where ACP compares them to the manifest in the signed policy. The snapshot is also observation only: restoring a file does not clear containment.

## What agents never do

```text
request, print, copy or compile ACP, GitHub, Railway, provider or policy-signing secrets
use repository content, clones, forks, alternate agents, or alternate tools to
  manufacture an owner override
delegate a refused action to another agent, tool or client to obtain a different answer
retry a DENY, REVERIFY_REQUIRED, QUARANTINE or LOCKED decision in a loop
attempt destructive repair after a trust failure without a RECOVERY_AUTHORIZED decision
edit contracts/acp-protected-effects.yaml or the profile to reclassify an effect as unprotected
```

A refused remote decision that the owner wants overridden requires a signed,
scope-matched grant in shared ACP state; a deviation record alone does not
authorize it. The local opt-out above applies only to local/preview client
checks and cannot override a remote ACP decision.

## Scope of what this reaches

ACP gates what asks it. A client that never calls `/internal/authorize` still pushes, merges and deploys. Enforcement outside the agent (branch protection, server-side hooks, a deploy gate that verifies an ACP capability) is a separate control and is recorded as a provider binding when present. Do not claim enforcement that only the agent-side instruction provides.

## Onboarding a repository

Formal Project Bootstrap is the canonical ACP client, project binding, checks, and policy-proposal generator for new projects. Do not create a project-specific ACP client when starting a project with this bootstrap. A policy proposal is only input to gateway registration; it does not register, sign, allowlist, deploy, or authorize anything by itself.

For an existing project, run `python3 scripts/reconcile-project.py --project /path/to/project --pretty` before adoption. Its `acpIntegration` result identifies a bootstrap-owned integration, a likely existing or legacy integration, or no detected integration, and lists only the paths used as evidence. An endpoint set marked `configured` means only that its name exists in the machine registry; `credentialAuthentication: NOT_VERIFIED` means the bearer has not been checked. Run `python3 scripts/acp-check.py --json` from the target project to verify endpoint health, authentication, board visibility, and whether an active policy governs the repository. Treat inventory as discovery, not proof of policy activation. Preserve any existing policy and enforcement while reconciling. If the project has a legacy client, migrate it through `docs/EXISTING_PROJECT_ADOPTION.md`; do not run two competing clients or change gateway policy as an incidental part of local adoption.

Four steps; project-local setup creates the binding and proposal, while gateway registration and activation remain operator-controlled.

```bash
# 1. in the bootstrap: bind (never writes the token)
#    the project selects the machine's named endpoint set
python3 scripts/init-project.py --name "My Project" --id my-project --output ../my-project \
    --acp-endpoint-set production \
    --acp-owner my-org --acp-project 3 \
    --acp-repository my-org/my-project

# 2. configure ACP_GATEWAY_TOKEN through the approved secret store into the project's
#    ignored .env.local or process environment; never place it in command arguments
#    or committed files. Then confirm the board is readable through ACP.
cd ../my-project
python3 scripts/acp.py board

# 3. review this proposal and submit it to the currently configured gateway
#    operator's documented registration flow (dry run first).
cat .formal-bootstrap/acp-policy-proposal.json

# 4. after the policy is active on the bound deployment, in the project
python3 scripts/acp-check.py --record
```

The registration command and deployment target are intentionally not hard-coded here: they belong to the active gateway deployment. Do not use a legacy Railway-only registration path when the project is bound to a different active gateway deployment. Registration must validate and sign the policy with the gateway's configured authority, preserve existing policies, activate the policy on the intended deployment, and verify the resulting repository and board binding.

For complete local ACP wiring, pass a named machine endpoint set (default `production`), an existing ProjectsV2 board (`--acp-owner` and `--acp-project`), and the GitHub repository identity (`--acp-repository OWNER/NAME`). The initializer writes only board and repository identity to the committed-safe `.agents/board.env`; the profile names the endpoint set and the machine-local registry supplies endpoint URLs and Keychain account names. It generates `.formal-bootstrap/acp-policy-proposal.json` from the generated control files. The proposal is excluded from compiled context and is only a draft: it grants no authority and must be regenerated after protected control files change.

Repository identity is sufficient to generate a boardless proposal before an endpoint set or board exists. Missing gateway or board bindings leave the profile `declared` and initialization prints `ACP_SETUP_INCOMPLETE` with the exact setup steps. Without any repository identity or ACP inputs, it prints `ACP_SETUP_NOT_CONFIGURED` and points to this onboarding section. A `bound` profile status means local endpoint, board, and repository identities are wired; it does **not** mean a policy is registered or that the repository is governed. E0 observation and E1 isolated local edits/tests/commits remain available while setup is incomplete or ACP is unavailable. Protected pushes, merges, deployments, credential changes, policy activation, and recovery remain fail-closed.

`init-project.py` also writes `.env.example` and the `.gitignore` entries that keep `.env.local` out and `AGENTS.md`/`.agents/` in (a global `core.excludesFile` commonly ignores both, and only the repository's own `.gitignore` can override it). Prefer the endpoint-specific macOS Keychain accounts named in the machine registry. The initialized project prints the one-time machine setup and gateway registration, board-allowlist, and deployment steps that remain with the ACP operator.

`acp-check.py` reports endpoint configuration and health, production failover configuration, credential acceptance, repository identity and policy registration, resource authorization state, and board binding as separate onboarding states. It continues independent checks after a policy notice so the report can show multiple blockers together. The JSON `onboarding.unmet_conditions` entries identify the responsible role and next step for each known blocker; the unauthenticated repository notice is checked before bearer authentication so a rejected token does not hide policy state. A configured board is distinguished from one verified through the gateway. Resource authorization preserves `NORMAL`, `QUARANTINED`, `LOCKED`, `RECOVERY_AUTHORIZED`, and `VERIFY_RECOVERY`; it reports missing or unknown state as unverified instead of treating every non-normal state as containment. Containment and recovery go to a security administrator, while an unknown state goes to the gateway operator. It returns non-zero on any refusal and preserves the gateway's own remedy. Its result is recorded in `records/evidence.jsonl` as gate `B0-acp`.

`acp.py board` treats a legacy `acp.gate` value as the configured board-read mode, never as proof of a current lock. If an older gateway returns a successful board read without the explicit gate-state fields, the client labels that observation `BOARD_READ_ALLOWED`; it says only that this board read was not blocked. It does not establish permission for commits, pushes, merges, deployments, or other protected actions, which still require their own exact ACP `ALLOW`.

`acp.py policy` proposes a policy: the repository and canonical remote, the bound board (which gates the board read), and an artifact manifest: the project's control files — `AGENTS.md`, `NEW_AGENT_PROMPT.md`, `PROJECT_PROFILE.json`, `PROJECT_INTENT.md`, the bindings, the action map, this document, `.agents/board.env`, and the client scripts `acp.py`/`acp-check.py` themselves — at their current hashes with `change_policy: authorize_and_report` (`--strict` makes them `admin_exact_transition`, where an unexpected change quarantines the resource), plus two patterns: `.agents/skills/**` (`authorize_and_report`) and `records/deviations.jsonl` (`report_only`). The candidate grants nothing until an operator registers it; `register-policy.py` signs it when the gateway holds `ACP_POLICY_SIGNING_KEY`, keeps any board, notice, manifest or recovery grant a re-registration does not restate, and validates the whole registry with the gateway's own loader before writing.

When the vendored `github-projects` skill and this document are both present, the skill carries the wire protocol for board work and this document carries the authority rules.
