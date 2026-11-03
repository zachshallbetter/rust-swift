# Project README Standard

**Status:** Bootstrap documentation requirement  
**Scope:** Human-readable entry pages created or adopted by Formal Project Bootstrap.

A README is the project's entry point, not a second authority system. It should orient a reader, point to the canonical sources, and report only what the repository can support.

## Required content

Every project README SHOULD contain:

1. One descriptive top-level title and a concise, factual opening summary.
2. The project's purpose and scope, linked to `.agents/project/intent.md` where present.
3. A quick start or clear first-reading path. List only commands that exist and are appropriate for the project.
4. A repository map that identifies canonical specifications, implementation, records, and tests when those exist.
5. Verification instructions that distinguish structural checks, implementation tests, and empirical evaluation.
6. Current status, material limitations, and non-goals. Do not infer implementation, test, review, or empirical status from plans or scaffolding.
7. License and citation information when the project declares them; do not invent either.

Use a single `#` heading. Keep headings descriptive, avoid duplicate title headings, and retain project-specific terminology where it improves accuracy. A protocol, research corpus, library, and application need not use identical section names or claim the same kinds of maturity.

## Generated project entry page

For a new project, the initializer creates an entry page with links to intent, profile, bootstrap sequence, and the validator. It describes the bootstrap shell only; project-specific behavior, commands, licensing, and empirical status remain for the project owner to document from evidence.

## Existing-project adoption

Existing README files are preserved. Validation reports missing entry-point elements as warnings in existing-project mode. Reconcile or edit the README under the project's existing authority; bootstrap does not overwrite it to satisfy this standard.

The standard is a quality floor, not a fixed template. Keep terminology and section order appropriate to the project, and do not add headings or claims that have no supporting project content.
