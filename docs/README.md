# `rust-swift` Documentation Index

This directory contains the canonical architecture specifications, formal contracts, and governance standards for the `rust-swift` compiler workspace.

---

## Part I: Compiler Architecture & Specifications

The core compiler design and platform strategy follow a sequential, canonical series:

| Document | Title | Purpose & Scope |
|---|---|---|
| **[`01-product-definition.md`](01-product-definition.md)** | Product Definition | Core value proposition, success criteria, non-goals, and operating modes. |
| **[`02-architecture.md`](02-architecture.md)** | Compiler Pipeline Architecture | End-to-end stage breakdown: `syn` parser $\to$ Semantic IR $\to$ Swift AST $\to$ SPM generator. |
| **[`03-semantic-contract.md`](03-semantic-contract.md)** | Semantic Lowering Contract | Normative invariants governing memory safety, arithmetic modes, and FSM transition totality. |
| **[`04-language-matrix.md`](04-language-matrix.md)** | Language Matrix & Boundary | Admitted Rust grammar boundary and deterministic diagnostic codes (`E0001`–`E0015`). |
| **[`05-validation.md`](05-validation.md)** | Qualification & Differential Harness | 16-suite differential test harness, failure mutation injection, and assertion counts ($\ge 20$). |
| **[`06-implementation-plan.md`](06-implementation-plan.md)** | Implementation Plan | Phased delivery roadmap, crate boundaries, and verification milestones. |
| **[`07-rfc.md`](07-rfc.md)** | Zero-FFI Source Translation RFC | Foundational RFC establishing pure ahead-of-time source-to-source translation without C-ABI wrappers. |
| **[`08-platform-architecture-matrix.md`](08-platform-architecture-matrix.md)** | Multi-Platform Architecture Decision Matrix | Comparative evaluation across **iOS, macOS, tvOS, watchOS, visionOS, and Android** vs. UniFFI, egui, and Tauri. |
| **[`09-apple-ecosystem-capabilities-and-kits.md`](09-apple-ecosystem-capabilities-and-kits.md)** | Apple Ecosystem Capabilities & Kits | In-depth architecture covering Apple signing identities, sandbox/entitlements, and system frameworks (AppKit, FSKit, RealityKit, WidgetKit, FileProvider). |

---

## Part II: Governance, Epistemic Integrity & Bootstrap

The workspace is governed by the [Formal Project Bootstrap](https://github.com/zachshallbetter/formal-project-bootstrap) standard, enforcing epistemic integrity and strict claim taxonomy ($\text{Documented} \neq \text{Implemented} \neq \text{Tested} \neq \text{Empirically Validated}$).

| Category | Documents | Description |
|---|---|---|
| **Epistemic Authority** | [`AUTHORITY_MODEL.md`](AUTHORITY_MODEL.md)<br>[`FORMAL_RESOURCES.md`](FORMAL_RESOURCES.md)<br>[`NEGATIVE_KNOWLEDGE.md`](NEGATIVE_KNOWLEDGE.md) | Order of precedence, pinned formal manifests, and permanent retention of disproven hypotheses. |
| **Verification & Evidence** | [`EVIDENCE_METHOD.md`](EVIDENCE_METHOD.md)<br>[`LANDING_AND_PROMOTION.md`](LANDING_AND_PROMOTION.md)<br>[`CONFORMANCE.md`](CONFORMANCE.md) | Cryptographic proof dossiers, mutation gate verification, and branch landing rules. |
| **Execution & Agents** | [`BOOTSTRAP_SEQUENCE.md`](BOOTSTRAP_SEQUENCE.md)<br>[`EXECUTION_PIPELINE.md`](EXECUTION_PIPELINE.md)<br>[`MULTI_AGENT_COORDINATION.md`](MULTI_AGENT_COORDINATION.md)<br>[`WORK_GRAPH.md`](WORK_GRAPH.md) | Autonomous agent execution contracts, task claiming, and work graph isolation. |
| **Protected Effects** | [`ACP_INTEGRATION.md`](ACP_INTEGRATION.md)<br>[`EXECUTION_PROVIDER_CONTRACT.md`](EXECUTION_PROVIDER_CONTRACT.md) | External authorization gateway, governed effects (push/merge/tag), and fail-closed security. |
| **Context & Evolution** | [`CONTEXT_COMPILATION.md`](CONTEXT_COMPILATION.md)<br>[`PROTOCOL_ADAPTERS.md`](PROTOCOL_ADAPTERS.md)<br>[`UPGRADING.md`](UPGRADING.md)<br>[`RECOVERY.md`](RECOVERY.md) | Deterministic `llms.txt` generation, context locks, and safe state recovery procedures. |
