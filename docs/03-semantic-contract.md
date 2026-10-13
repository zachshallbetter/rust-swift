Rust → Swift Compiler \| Proposal v0.1 \| 13 October 2026

## Observation model

Define behavior as return values, errors, traps, emitted bytes, resource effects, and ordered traces at declared observation boundaries. Timing and allocation are excluded unless explicitly contracted.

## Integer semantics

Specify signed/unsigned widths, checked/wrapping/saturating/overflowing operations, casts, shifts, and panic/trap conditions per operation. Never infer wrapping from ordinary addition.

## Strings

Distinguish UTF-8 byte offsets, Unicode scalar values, and grapheme clusters. Validate slicing boundaries, normalization assumptions, and string comparison semantics.

## Ownership

Track move, shared borrow, exclusive borrow, aliasing, and destruction. A Swift struct, class, or inout mapping is a lowering candidate, not proof of equivalence.

## Collections

Specify ordering, equality, capacity/pointer observations, hashing assumptions, and mutation effects. Do not substitute Swift collections when observable semantics differ.

## Errors and panics

Keep Result as data in compatibility mode. Preserve panic/trap boundaries explicitly; Swift throws is an optional ergonomic facade.

## Qualification states

Documented, implemented, compiled, tested, empirically validated, and formally proved are separate claims. Each claim requires its own evidence.
