---
description: Type system architecture, explicit failure returns, safe arithmetic, and hostile input handling
globs: ["**/*"]
---

# Type System Architecture & Domain Invariants

This document defines the type invariants, numerical representations and error handling contracts for projects operating under the Formal Project Bootstrap doctrine. The principles apply to every stack; use the project's own idioms to express them.

---

## 1. Explicit Failure Types

Library and production code must model fallibility in its types or return values instead of relying on implicit crashes.

- **Explicit results**: every operation that can meet missing data, malformed input, a full disk or an unsupported feature returns a result type, optional, or typed error, using the language's own mechanism.
- **Domain-specific errors**: each module or package defines its own error type. Do not use bare strings or untyped catch-all errors in public APIs.
- **Unsupported features**: unfinished features return an explicit "unsupported" error rather than a placeholder that throws or aborts.
- **No forced extraction**: do not force-unwrap, assert non-null, or swallow exceptions on a path reachable from foreign input. Handle the failure or return it.

---

## 2. Hostile Numerical Types & Safe Arithmetic

All numerical data received from files, foreign protocols, IPC sockets or user parameters is hostile:

- **Safe indexing**: do not index or slice with input-derived numbers without a bounds check. Check string slice positions against character boundaries.
- **Checked arithmetic**: byte lengths, offsets, coordinates and buffer capacities use checked or saturating arithmetic where the language can overflow or wrap. Guard against division by zero.
- **Floating point**: values for dimensions, coordinates, scale factors and layout bounds are verified finite. Reject or clamp NaN and infinities before converting to integers.

---

## 3. Geometric & Spatial Primitives

Where the project does 2D/3D work, keep screen space, canvas space and world space distinct:

- **Typed coordinates**: use strongly typed coordinate math from a vetted library. Keep physical device pixels, logical layout points and normalized texture coordinates separate.
- **Non-invertible transforms**: check a matrix for invertibility before computing an inverse projection.
- **Bounding boxes**: keep them normalized (`min <= max`); never pass an inverted volume to a spatial query structure.
