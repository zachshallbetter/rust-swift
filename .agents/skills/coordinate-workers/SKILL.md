---
name: coordinate-workers
description: Use when two or more workers operate concurrently, exchange findings, hand off work, supersede approaches, or need shared namespace allocation.
---

# Coordinate Workers

1. Allocate disjoint workspaces and mutable scopes.
2. Reserve scarce namespaces.
3. Route evidence-bearing AgentMessages.
4. Revalidate work whose assumptions were invalidated by peers.
5. Transfer claims explicitly on handoff.
6. Preserve superseded/reverted work as evidence when useful.
7. Serialize project-wide state transitions.
8. Never treat peer consensus as authority.
