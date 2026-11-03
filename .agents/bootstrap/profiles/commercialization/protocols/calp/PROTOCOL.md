# Commercial Asset Lifecycle Protocol (CALP)

> Bootstrap v0.2.0 note: this draft is retained as a **commercialization profile overlay**. It is not part of the generic project kernel.


**Status:** Draft  
**Version:** 0.1.0  
**Date:** 2026-08-27

## 1. Purpose

The Commercial Asset Lifecycle Protocol (CALP) defines how a reusable intellectual artifact becomes a qualified semantic asset, how that asset may be projected into a commercial surface, and what evidence and authority are required to change its commercial standing.

CALP governs **one asset at a time**.

It answers:

> What is this asset, what can legitimately be claimed about it, what is its current commercial standing, and what lawful transitions may change that standing?

CALP does not execute implementation work, qualify evidence independently, govern UI morphology, or decide strategic portfolio allocation.

---

## 2. Governing Principle

> **Commercialization is a qualified, authorized projection of a semantic asset—not a property an artifact can acquire merely by being useful, implemented, popular, or profitable.**

A second rule follows:

> **Productization is a promotion state, not an act of enthusiasm.**

---

## 3. Protocol Boundaries

CALP owns:

- commercial asset identity;
- asset claims;
- asset lifecycle standing;
- reusable-boundary qualification;
- commercial eligibility predicates;
- commercial surface semantics;
- commercial promotion;
- protected commercial effects;
- commercial transition receipts.

CALP imports or delegates:

- **EQP** — evidence qualification;
- **IEPE** — authorized execution;
- **ICP** — pilots, contracts, consequential commercial interactions, receipts and recovery;
- **SES** — customer-facing semantic projection;
- **PDP** — consequential decisions under uncertainty;
- **Authority Model** — protected-effect authority;
- project-specific systems — implementation.

CALP MUST NOT enlarge the authority of an imported protocol.

---

## 4. Core Distinctions

CALP requires the following to remain separately representable:

```text
ARTIFACT
    !=
ASSET
    !=
QUALIFIED ASSET
    !=
DEFENDED ASSET
    !=
COMMERCIAL SURFACE
    !=
PRODUCT
    !=
COMMERCIALIZED PRODUCT
    !=
SUCCESSFUL BUSINESS
```

### Artifact

A concrete work product such as a repository, runtime, protocol, document, library, dataset, benchmark, model, design system, application, or tool.

### Asset

A stable semantic identity representing reusable intellectual value that may have one or more implementations or projections.

### Qualified Asset

An asset whose identity, provenance, ownership posture, reusable boundary, and required evidence have been sufficiently established for its declared standing.

### Defended Asset

A qualified asset for which a material claim has reproducible evidence with explicit scope and limitations.

### Commercial Surface

A bounded market-facing projection of one or more assets.

### Product

A commercial surface with defined delivery semantics, versioning, support, limits, and economic terms.

### Commercialized Product

A product for which a binding commercial commitment exists and pricing, support, renewal or termination, and operating boundaries are defined.

### Successful Business

A portfolio-level economic conclusion. Commercialization alone does not establish it.

---

## 5. Canonical Asset Identity

The semantic asset is independent of any one repository, package, deployment, website, or product.

```ts
interface AssetIdentity {
  assetId: string;
  canonicalName: string;
  revision: string;

  kind:
    | "method"
    | "protocol"
    | "engine"
    | "runtime"
    | "library"
    | "tool"
    | "dataset"
    | "design-system"
    | "model"
    | "workflow"
    | "application"
    | "research"
    | "other";

  originRefs: SourceRef[];
}
```

Example:

```text
                  ASSET
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
     Git repo     package      hosted API
        │
        ▼
     protocol
        │
        ▼
 enterprise integration
```

The asset remains semantically stable while its realizations may change.

---

## 6. Asset Claims

An asset record is a set of individually qualified claims, not a marketing narrative.

```ts
interface AssetClaim {
  id: string;
  assetRef: AssetRef;

  claim: string;

  class:
    | "semantic"
    | "implementation"
    | "performance"
    | "reuse"
    | "ownership"
    | "market"
    | "commercial"
    | "operational";

  evidenceRefs: EvidenceRef[];

  qualification:
    | "observed"
    | "documented"
    | "tested"
    | "defended"
    | "bounded"
    | "contradicted"
    | "unknown";

  permittedProjections: ProjectionRef[];
  caveats: string[];
}
```

### Claim Projection Invariant

A downstream surface may omit, shorten, or restate a claim without altering meaning.

It may not:

- strengthen the qualification state;
- erase limitations;
- convert correlation to causation;
- generalize beyond the tested domain;
- convert revenue into technical validation;
- convert internal reuse into external market validation.

---

## 7. Multi-Dimensional Asset State

CALP MUST NOT compress lifecycle truth into one universal maturity field.

```ts
interface AssetState {
  semantic: SemanticMaturity;
  implementation: ImplementationMaturity;
  evidence: EvidenceMaturity;
  commercial: CommercialMaturity;
  operational: OperationalMaturity;
  protection: ProtectionPosture;
}
```

### Semantic Maturity

```text
observed
candidate
canonical
superseded
```

### Implementation Maturity

```text
documented
implemented
tested
empirically_validated
```

### Evidence Maturity

```text
unqualified
pending_proof
defended
bounded
contradicted
```

### Commercial Maturity

```text
none
candidate
pilot
contracted
commercialized
```

### Operational Maturity

```text
inactive
live
maintained
scaling
dormant
retiring
retired
```

### Protection Posture

```text
unresolved
retained_private
trade_secret
copyright
open
provisional_candidate
filed
licensed
```

These dimensions are orthogonal.

Example:

```text
semantic:       canonical
implementation: tested
evidence:       defended
commercial:     candidate
operational:    inactive
protection:     retained_private
```

A projection MUST NOT silently collapse this into an unsupported phrase such as "production ready."

---

## 8. Canonical Relations

### Semantic and technical

```text
DERIVED_FROM
GENERALIZES
SPECIALIZES
IMPLEMENTS
REALIZES
DEPENDS_ON
REUSES
SUPERSEDES
```

### Evidence

```text
EVIDENCED_BY
QUALIFIED_BY
CONTRADICTS
```

### Commercial

```text
PROJECTED_AS
OFFERED_AS
LICENSED_AS
OPERATED_AS
GENERATES_REVENUE
INCURS_COST
REQUIRES_SUPPORT
```

### Lifecycle

```text
RETIRES
REPLACED_BY
```

A commercial relation does not mutate the underlying semantic identity.

---

## 9. Asset Discovery

Agents may discover candidate assets.

Agents may not self-promote them.

```text
repository activity
PRs
commits
dependencies
repeated implementation
research
customer requests
protocol reuse
support patterns
        ↓
AssetObservation
        ↓
candidate detector
        ↓
CandidateAsset
```

Example observation:

> A materially similar context-compilation mechanism appears independently in three projects.

This may establish recurrence.

It does not establish commercial value.

```ts
interface AssetObservation {
  observationId: string;
  sourceRefs: SourceRef[];
  observedAt: string;

  subject?: AssetRef;
  candidateName?: string;

  assertion: string;
  recurrenceEvidence?: RecurrenceEvidence;

  observer: ActorRef;
  provenance: ProvenanceRef;
}
```

### Observation Invariant

> **Observation does not grant semantic, ownership, qualification, publication, pricing, or commercialization authority.**

---

## 10. Qualification

CALP delegates qualification to EQP.

```text
CandidateAsset
      ↓
CALP qualification request
      ↓
EQP
      ↓
EvidenceBundle
Qualification
Limitations
Unknowns
      ↓
CALP eligibility predicate
```

Typical dimensions include:

```text
identity
provenance
reuse
separability
ownership
implementation
measured effect
dependency boundary
failure behavior
```

Strong asset evidence does not imply market evidence.

---

## 11. Commercial Eligibility

### Commercial Evaluation

```text
EligibleForCommercialEvaluation(asset) :=
    identity_resolved
    ∧ ownership_resolved
    ∧ reusable_boundary_exists
    ∧ evidence_not_contradicted
```

### Pilot

```text
EligibleForPilot(asset) :=
    EligibleForCommercialEvaluation(asset)
    ∧ buyer_defined
    ∧ problem_defined
    ∧ measurable_baseline_defined
    ∧ surface_contract_defined
    ∧ risk_within_allowed_scope
    ∧ pilot_authority_granted
```

### Commercialization

```text
Commercialized(surface) :=
    executed_commercial_agreement
    ∧ pricing_defined
    ∧ support_boundary_defined
    ∧ renewal_or_termination_defined
    ∧ operational_responsibility_defined
```

Commercialization does not establish general market fit, technical superiority, or generalizability.

---

## 12. Commercial Surface Contract

A commercial surface is a governed projection.

```ts
interface CommercialSurfaceContract {
  id: string;
  revision: string;

  assetRefs: AssetRef[];

  buyer: BuyerClass;
  problem: ProblemDefinition;

  delivery:
    | "api"
    | "library"
    | "cli"
    | "saas"
    | "license"
    | "oem"
    | "data"
    | "service"
    | "application";

  inputs: ContractValue[];
  outputs: ContractValue[];

  preservedClaims: ClaimRef[];
  excludedClaims: ClaimRef[];

  pricingUnit: PricingUnit;

  supportBoundary: SupportContract;
  securityBoundary: SecurityContract;
  dataBoundary: DataContract;

  versioning: VersionPolicy;
  deprecation: DeprecationPolicy;

  dependencies: DependencyRef[];
  exclusions: string[];
}
```

### Surface Heuristic

```text
Repeated COMPUTATION
→ API

Reusable IMPLEMENTATION
→ library / SDK / CLI

Persistent WORKFLOW
→ SaaS

Infrastructure CAPABILITY
→ enterprise license / OEM

Formal METHOD
→ protocol + conformance/tooling

Structured KNOWLEDGE
→ data / benchmark subscription

Expert PROCESS
→ productized service

Consumer EXPERIENCE
→ standalone application
```

This is a heuristic, not authority.

---

## 13. Pilot Profile

A commercial pilot is an ICP interaction profile.

Its purpose is to resolve named commercial uncertainty.

```yaml
pilot:
  asset: asset.example@0.3

  hypothesis:
    "The asset materially improves a specified baseline."

  participant_roles:
    - provider
    - customer
    - evaluator

  baseline:
    metrics:
      - cycle_time
      - human_interventions
      - rework_rate
      - cost_per_qualified_output

  target:
    intervention_reduction: ">= 40%"
    qualification_rate: ">= baseline"

  boundary:
    environment: staging
    data_scope: specified
    duration: 30d

  kill:
    - material_security_failure
    - support_hours_above_budget
    - no_measurable_delta

  evidence:
    evaluator: independent
    reproduction: required

  authority:
    commercial_commitment: human
```

ICP owns the attempt, participants, authority checks, transitions, outcomes, recovery, and receipt.

CALP owns the resulting commercial standing.

---

## 14. Productization Execution

Authorized productization work is delegated to IEPE.

```text
CALP Promotion Decision
       ↓
authorized productization intent
       ↓
IEPE
       ↓
epic
issue graph
bounded work
artifact
evidence
qualification
promotion
```

A productization issue SHOULD bind:

```text
asset identity
commercial surface
scope
exclusions
dependencies
acceptance criteria
evidence requirements
budget
permissions
stop conditions
authority references
```

An issue may authorize implementation work without authorizing protected commercial effects.

---

## 15. Authority Classes

```text
AssetRegistrationAuthority
SemanticPromotionAuthority
OwnershipAssertionAuthority
EvidenceQualificationAuthority
CommercialEvaluationAuthority
PilotCommitmentAuthority
PublicationAuthority
PricingAuthority
ContractAuthority
OperationalAuthority
RetirementAuthority
```

### Fundamental Distinction

```text
capability
!=
permission
!=
qualification authority
!=
commercial commitment authority
```

### No Authority by Coalition

> **Two or more agents may pool evidence, information, proposals, and work capacity. Their combined authority shall never exceed the union of explicitly delegated scopes, and no protected effect becomes authorized merely because multiple agents agree.**

Agreement may be evidence.

It is not authority.

---

## 16. Protected Commercial Effects

CALP SHOULD fail closed around:

```text
declare ownership
publish previously private IP
publish consequential commercial claims
enter paid pilot
materially change price
execute customer contract
accept unusual liability
grant exclusive license
expose customer data
incur spend above threshold
transfer IP
retire customer-facing product
delete canonical evidence
```

Routine reversible work may be delegated broadly.

---

## 17. Commercial Receipts

Every consequential lifecycle transition produces an append-only receipt.

```text
ObservationReceipt
QualificationReceipt
PromotionReceipt
PilotReceipt
PublicationReceipt
CommercialCommitmentReceipt
OperationReceipt
RetirementReceipt
```

Example:

```json
{
  "type": "commercial-promotion",
  "asset": "asset.context-compiler@0.3",
  "from": {
    "commercial": "candidate"
  },
  "to": {
    "commercial": "pilot"
  },
  "basis": [
    "EQP-331",
    "SURFACE-context-compiler-01",
    "PILOT-042"
  ],
  "authority": "commercial-evaluation-authority",
  "limitations": [
    "enterprise market not established",
    "evidence limited to software repositories"
  ]
}
```

A later contradiction creates a new event. It does not rewrite historical receipts.

---

## 18. Reversibility

### Reversible

```text
draft change
unpublished price experiment
internal package revision
reversible deployment
```

### Compensable

```text
refund
credit
service rollback
contract termination under declared terms
```

### Durable

```text
public IP disclosure
executed assignment
exclusive license
regulatory filing
irreversible data disclosure
```

Authority requirements SHOULD increase with durability.

---

## 19. Conformance Requirements

### C-01 Candidate Cannot Self-Promote

Agent discovery alone MUST NOT permit candidate → canonical promotion.

### C-02 Unclear Ownership Blocks Exposure

If ownership is unresolved:

```text
private investigation = potentially allowed
public publication = blocked
pilot = blocked
commercialization = blocked
```

### C-03 Revenue Does Not Equal Validation

Payment may establish willingness-to-pay evidence.

It does not establish technical superiority, general market fit, or generalizability.

### C-04 Projection Cannot Strengthen Claim

Canonical:

```text
"Reduced intervention in tested software repositories."
```

Invalid projection:

```text
"Eliminates agent supervision in enterprise engineering."
```

### C-05 Contradiction Blocks Dependent Promotion

If required evidence is contradicted, promotion depending upon that claim is blocked.

---

## 20. Protocol Manifest

Illustrative only; exact imported versions must be pinned in implementation.

```yaml
protocol:
  id: commercial-asset-lifecycle
  short: CALP
  version: 0.1.0

authority:
  normative:
    - PROTOCOL.md
    - TERMINOLOGY.md
    - schemas/

imports:
  iepe:
    bindings:
      ExecutionAuthority: constrained
      IssueContract: exact
      Promotion: constrained

  icp:
    bindings:
      InteractionContract: exact
      Receipt: exact
      Recovery: exact

  eqp:
    bindings:
      Evidence: exact
      Qualification: exact

  ses:
    bindings:
      Projection: constrained
      SemanticIdentity: exact

  pdp:
    bindings:
      DecisionAuthority: constrained

exports:
  - AssetIdentity
  - AssetClaim
  - AssetState
  - CommercialSurface
  - CommercialEligibility
  - CommercialPromotion
```

---

## 21. Recommended Repository Shape

```text
commercial-asset-lifecycle/
├── PROTOCOL.md
├── TERMINOLOGY.md
├── protocol-manifest.json
│
├── schemas/
│   ├── asset-record.schema.json
│   ├── asset-claim.schema.json
│   ├── commercial-surface.schema.json
│   ├── commercial-promotion.schema.json
│   └── commercial-receipt.schema.json
│
├── profiles/
│   ├── discovery.profile.json
│   ├── qualification.profile.json
│   ├── pilot.profile.json
│   └── operating.profile.json
│
└── conformance/
    ├── candidate-cannot-self-promote.json
    ├── unclear-ownership-blocks-exposure.json
    ├── money-does-not-equal-validation.json
    ├── projection-cannot-strengthen-claim.json
    └── contradicted-proof-blocks-promotion.json
```

---

## 22. CALP Invariants

1. Artifact existence does not establish asset standing.
2. Observation does not grant promotion authority.
3. Commercialization does not imply validation.
4. Revenue is evidence of economic commitment, not technical correctness.
5. Ownership uncertainty blocks protected exposure.
6. Generated context is downstream from canonical authority.
7. Commercial projections may not strengthen canonical claims.
8. Negative and contradictory evidence is retained.
9. An asset and a commercial surface are distinct identities.
10. Agent agreement does not create authority.
11. Protected durable effects fail closed.
12. Commercial standing remains independently representable from implementation standing.

---

## 23. Worked Example — Context Compiler

A recurring mechanism appears across multiple formal engineering environments:

```text
protocols
architecture
schemas
PRDs
issues
evidence
exceptions
        ↓
   CONTEXT COMPILER
        ↓
bounded agent context
```

Initial standing:

```text
semantic:       candidate
implementation: documented
evidence:       unqualified
commercial:     none
operational:    inactive
```

After boundary extraction, ownership resolution, implementation evidence, and EQP qualification:

```text
semantic:       canonical
implementation: tested
evidence:       defended
commercial:     candidate
operational:    inactive
```

A first commercial surface might be:

```text
the project Context Compilation Pilot
```

with:

```text
inputs:
repository
architecture docs
development policies
issue workflow

outputs:
formal source graph
authority resolution
project context lock
compiled agent context
benchmark report
```

A paid pilot can establish external use and willingness to pay.

It does not establish broad market fit.

After an executed agreement plus defined pricing, support, renewal/termination, versioning, and operating boundaries:

```text
commercial: commercialized
```

becomes legitimate.

---

## 24. Success Condition

CALP succeeds when reusable intellectual work can move from observation to commercial operation without:

- inventing unsupported maturity;
- losing ownership provenance;
- conflating implementation with validation;
- allowing agents to manufacture their own authority;
- strengthening claims during projection;
- treating payment as proof of technical correctness.

The result is a qualified commercial asset whose standing can be explained, reproduced, audited, and lawfully changed.

