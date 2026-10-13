Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## Pipeline

Pinned Rust frontend -\> typed source and control-flow facts -\> target-neutral semantic IR -\> lowering -\> Swift AST/emitter -\> swiftc -\> qualification.

## Frontend choice

Prototype a pinned rustc driver for type resolution, trait selection, and macro expansion. Treat rustc internals as unstable. Keep a small independent fixture parser only for exploratory spikes, not as the authoritative compiler.

## Semantic IR

Represent typed values, source spans, ownership and regions, effects, control flow, drop obligations, integer modes, string units, and resolved calls. Preserve provenance for diagnostics.

## Backend

Generate deterministic Swift modules with explicit namespacing, stable file ordering, runtime imports only when required, and a source-to-generated symbol map.

## Runtime

Introduce compatibility shims only for mismatched semantics. Version and test each shim independently; prefer native Swift where equivalence is established.

## Reproducibility

Pin rustc and Swift toolchains, platform triple, feature flags, Unicode versions where relevant, build mode, and compiler revision in a manifest.
