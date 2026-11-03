# Bootstrap Upgrades

## The short version

```bash
.agents/bootstrap/bootstrap upgrade            # dry run: what the latest release would change
.agents/bootstrap/bootstrap upgrade --commit   # fetch, verify, apply, and commit exactly the upgrade
.agents/acp/acp push                           # publish it through ACP
```

`upgrade` reads the source repository from `.agents/project/installation.json` and fetches the target tag (default: the latest; `--to vX.Y.Z` to choose) into `~/.cache/formal-project-bootstrap/`. Before using it, it verifies two things:
- **The tag's signature**, against `gpg.ssh.allowedSignersFile`. `--allow-unverified` accepts an unverifiable one knowingly.
- **Every machinery file**, against the release's `RELEASE.json`.

`--from DIR` uses a local release checkout instead, for offline work. It refuses while bootstrap machinery has uncommitted edits (`--force` overrides). With `--commit` it stages only the upgrade's files and a lock compiled from that index (`gen-context.py --staged`), and leaves the project's other work in progress alone.

- `bootstrap repair` restores edited or missing machinery to the installed release, backing up the edited copies. A path listed in `bootstrapOverrides` is left alone.
- `bootstrap uninstall` is a dry run unless `--apply` is given. It removes the machinery and the unmodified shipped skills, and keeps `.agents/project/`, `.agents/records/` and modified skills. `--export DIR` also copies those out. Everything removed is backed up.

## Moving to the contained layout (0.7)

Before 0.7, a project kept bootstrap's files at its root and in `scripts/`, `schemas/`, `docs/`, `contracts/`, `bindings/` and `records/`. The first upgrade to 0.7 does four things:

1. **Moves each file under `.agents/`.** Machinery goes to `.agents/acp/` and `.agents/bootstrap/`. The project's own files go to `.agents/project/`, and the ledgers to `.agents/records/`.
2. **Rewrites the known old paths quoted in `AGENTS.md` and `.agents/project/`**, for example `PROJECT_PROFILE.json` → `.agents/project/profile.json`.
3. **Reports, without editing, every other project file that still quotes an old path**, with the replacement for each line. That covers CI workflows, Makefiles, READMEs, hooks and `core.hooksPath`.
4. **Leaves everything else in place.** A project's own `scripts/`, `docs/` and `records/` files are not bootstrap's and stay where they are.

The installed version moves from the profile to `.agents/project/installation.json`, so later upgrades no longer rewrite the profile. Rollback undoes the moves, the rewrites and the additions.

The rest of this document describes the underlying `upgrade-project.py` that `bootstrap upgrade` drives.

An upgrade is a reconciliation operation, not a template overwrite. The upgrade tool compares an installed project with a target bootstrap release, preserves project-owned changes, stages conflicts, and validates the result before reporting success.

## Plan

Run the read-only plan from the target release or an installed project:

```bash
python3 .agents/bootstrap/scripts/upgrade-project.py \
  --project /path/to/project \
  --from /path/to/formal-project-bootstrap-release \
  --base /path/to/previous-formal-project-bootstrap-release \
  --check
```

`--base` enables three-way classification. When it is omitted and the release is a Git checkout, the tool uses the release's own tag for the installed version (`v<bootstrapVersion>`), so a plan from a bootstrap checkout needs no extra argument.

Files fall into two kinds:

- **Project-owned:** `AGENTS.md`, `.agents/project/intent.md`, `.agents/project/context-sources.json`, `.agents/project/formal-resources.json`, `.agents/project/bindings.yaml`, `.agents/project/rules/` and `contracts/`. A project specialises these after adoption. A file still identical to the base is updated. A file the project changed is kept, and the release copy is staged under `candidates/` for review.
- **Bootstrap machinery:** everything else the release ships, such as `scripts/`, `schemas/`, `.agents/skills/` and the copied docs. These are replaced. If the project had edited one, the plan names it under `replacedLocalEdits` and its previous copy stays in the backup.

A project that has deliberately taken over a machinery file lists it in `.agents/project/profile.json`, for example `"bootstrapOverrides": [".agents/bootstrap/scripts/gen-context.py"]`. Upgrades then treat that file as project-owned: it is kept and the release copy is staged for review.

`.agents/project/profile.json` is merged: the release's new defaults are added and every existing project value is kept.

## Apply

```bash
python3 .agents/bootstrap/scripts/upgrade-project.py \
  --project /path/to/project \
  --from /path/to/formal-project-bootstrap-release \
  --base /path/to/previous-formal-project-bootstrap-release \
  --apply
```

The tool stages a plan and backup under `.agents/.upgrades/<version>/` (ignored by Git), applies the update, regenerates context, and runs `validate-bootstrap.py --check-context`. A validation failure restores the backup. A short summary on stderr names what was replaced, what was kept for review and where each copy is; stdout stays the plan document.

Kept project-owned files do not block the upgrade. Review their candidates when convenient. Files the release no longer ships are reported and left in place. Project identity, intent, bindings, ACP board configuration, and other project-owned values remain authoritative.

## Verify and roll back

```bash
python3 .agents/bootstrap/scripts/upgrade-project.py --project /path/to/project --from /path/to/release --verify
python3 .agents/bootstrap/scripts/upgrade-project.py --project /path/to/project --from /path/to/release --rollback
```

Verification is read-only. Rollback restores the last recorded upgrade backup; it does not reset Git history or delete unrelated project changes.

ACP policy registration remains a separate operator-authorized step. If an upgrade changes protected ACP artifacts, regenerate the candidate policy, review its manifest, and register it through the gateway only after approval.
