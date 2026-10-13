---
name: authorize-protected-effect
description: Use before any governed effect (push to a protected ref, merge, deploy, history rewrite, default-branch change, transfer, credential or policy change) when the project binds acp-gateway. Obtains the ACP decision, performs only what it authorizes, and disposes of every other decision without stopping the project.
---

# Authorize Protected Effect

1. Resolve the operation to an ACP action via `contracts/acp-protected-effects.yaml`. Not governed → proceed under the work item's own permissions.
2. Identify: repository (from `origin`), ref, current (`old_sha`) and intended (`new_sha`) revision, requested action, work item, claim.
3. Credentials come from the environment or `.env.local`; `scripts/acp.py` reads them. Never print, log, or compile them.
4. Report first when `reportBeforeProtectedEffect`: `python3 scripts/acp.py report --event PROTECTED_EFFECT_PENDING --checkpoint PRE_PUSH --action <action>` (use the matching `PRE_*` checkpoint), and `python3 scripts/acp.py snapshot --checkpoint PRE_PUSH`. Reporting grants nothing.
5. Decide: `python3 scripts/acp.py authorize --action <action> [--ref R --old-sha X --new-sha Y] --work-item <id> --record > decision.json`. Exit `0` only for `ALLOW`.
6. Dispose on `decision` (and `reason`):
   - `ALLOW` → perform exactly `authorized_action` (same action/ref/SHA), once, within the capability TTL. Where the boundary checks, `python3 scripts/acp.py verify-capability --decision decision.json`. Then re-read live state.
   - `AUTH_REQUIRED` → complete the flow named in `auth`; retry once.
   - `RECOVERY_AUTHORIZED` → perform only the described transition, once; then `python3 scripts/acp.py recovery-complete --decision decision.json --result succeeded|failed --observed-sha <sha>`.
   - `DENY` / `REVERIFY_REQUIRED` / `VERIFY_RECOVERY` → do not proceed; record `AUTHORITY_MISSING` with the reason; release or park the claim; reselect.
   - `QUARANTINE` / `LOCKED` → do not proceed and do not repair; preserve candidate and evidence; global stop for this resource; prepare the decision packet.
   - `synthetic: true` → the client refused locally. `GATEWAY_UNAVAILABLE`: classify once, do not poll. `ACP_NOT_GOVERNING`: no policy names this repository; a setup blocker for the operator (`docs/ACP_INTEGRATION.md` § Onboarding), not a reason to proceed. `AUTHORIZED_ACTION_MISMATCH`: do not perform either action.
7. `--record` writes `decision`, `reason`, `request_id`, `policy_id`, `policy_version`, `repository_trust` to `records/evidence.jsonl`; carry the same fields into the work disposition. Never record `capability_token`.
8. Never delegate a refused action to another agent, tool, clone or client to obtain a different answer.
