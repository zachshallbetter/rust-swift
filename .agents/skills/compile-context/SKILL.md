---
name: compile-context
description: Use after canonical source changes or before consequential execution when generated context may be stale. Rebuilds and verifies deterministic project context.
---

# Compile Context

1. Correct canonical sources first.
2. Run `python3 scripts/gen-context.py`.
3. Run `python3 scripts/gen-context.py --check`.
4. Verify branch/commit/dirty state in `.agents/context-lock.json`.
5. Use `.agents/llms.txt` to locate relevant material.
6. Load only bounded excerpts for a work item.
7. Never patch generated context manually.
