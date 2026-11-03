# Bootstrap Upgrades

An upgrade is a reconciliation operation, not a template overwrite. The upgrade tool compares an installed project with a target bootstrap release, preserves project-owned changes, stages conflicts, and validates the result before reporting success.

## Plan

Run the read-only plan from the target release or an installed project:

```bash
python3 scripts/upgrade-project.py \
  --project /path/to/project \
  --from /path/to/formal-project-bootstrap-release \
  --base /path/to/previous-formal-project-bootstrap-release \
  --check
```

`--base` enables three-way classification. When it is omitted and the release is a Git checkout, the tool uses the release's own tag for the installed version (`v<bootstrapVersion>`), so a plan from a bootstrap checkout needs no extra argument.

Files fall into two kinds:

- **Project-owned:** `AGENTS.md`, `PROJECT_INTENT.md`, `CONTEXT_SOURCES.json`, `FORMAL_RESOURCE_MANIFEST.json`, `bindings/PROJECT_BINDINGS.yaml`, `.agents/rules/` and `contracts/`. A project specialises these after adoption. A file still identical to the base is updated. A file the project changed is kept, and the release copy is staged under `candidates/` for review.
- **Bootstrap machinery:** everything else the release ships, such as `scripts/`, `schemas/`, `.agents/skills/` and the copied docs. These are replaced. If the project had edited one, the plan names it under `replacedLocalEdits` and its previous copy stays in the backup.

`PROJECT_PROFILE.json` is merged: the release's new defaults are added and every existing project value is kept.

## Apply

```bash
python3 scripts/upgrade-project.py \
  --project /path/to/project \
  --from /path/to/formal-project-bootstrap-release \
  --base /path/to/previous-formal-project-bootstrap-release \
  --apply
```

The tool stages a plan and backup under `.formal-bootstrap/upgrades/<version>/` (ignored by Git), applies the update, regenerates context, and runs `validate-bootstrap.py --check-context`. A validation failure restores the backup. A short summary on stderr names what was replaced, what was kept for review and where each copy is; stdout stays the plan document.

Kept project-owned files do not block the upgrade. Review their candidates when convenient. Files the release no longer ships are reported and left in place. Project identity, intent, bindings, ACP board configuration, and other project-owned values remain authoritative.

## Verify and roll back

```bash
python3 scripts/upgrade-project.py --project /path/to/project --from /path/to/release --verify
python3 scripts/upgrade-project.py --project /path/to/project --from /path/to/release --rollback
```

Verification is read-only. Rollback restores the last recorded upgrade backup; it does not reset Git history or delete unrelated project changes.

ACP policy registration remains a separate operator-authorized step. If an upgrade changes protected ACP artifacts, regenerate the candidate policy, review its manifest, and register it through the gateway only after approval.
