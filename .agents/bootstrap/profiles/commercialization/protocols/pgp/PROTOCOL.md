# Portfolio Governance Protocol (PGP)

> Bootstrap v0.2.0 note: this draft is retained as a **commercialization profile overlay**. It is not part of the generic project kernel.


**Status:** Draft  
**Version:** 0.1.0  
**Date:** 2026-08-27

## 1. Purpose

The Portfolio Governance Protocol (PGP) defines how a portfolio of qualified and candidate commercial assets competes for finite capital, human attention, operational capacity, and execution priority.

PGP governs **relationships among assets**.

It answers:

> Given the current portfolio, evidence, risk, resources, and opportunity, which assets should receive work, which should remain available, and which should be constrained, combined, licensed, opened, made dormant, or retired?

PGP does not define asset identity or commercial qualification. Those belong to CALP.

---

## 2. Governing Principle

> **Portfolio disposition is an evidenced, authorized state transition—not the output of a hidden universal score.**

A second rule follows:

> **The portfolio may generate review work automatically. It may not manufacture the authority required to make consequential strategic commitments.**

---

## 3. Protocol Boundaries

PGP owns:

- portfolio identity and state;
- commercial asset relationships at portfolio scale;
- portfolio profiles;
- human-attention accounting;
- portfolio dispositions;
- review triggers;
- portfolio work-frontier generation;
- portfolio-level operating observations.

PGP imports or delegates:

- **CALP** — asset identity, asset claims, asset commercial standing;
- **EQP** — evidence qualification;
- **IEPE** — execution of authorized portfolio work;
- **PDP** — consequential weighting under uncertainty;
- **ICP** — external commercial interactions when required;
- **SES** — portfolio/admin projections;
- **Authority Model** — protected disposition authority.

---

## 4. Portfolio World

The portfolio is a formal operational world.

```text
PORTFOLIO
│
├── assets
├── commercial surfaces
├── customers
├── revenue
├── costs
├── support burden
├── dependencies
├── evidence
├── risk
├── human attention
├── opportunity
└── unresolved contradictions
```

Its state must be queryable without requiring a human to reconstruct it from memory.

---

## 5. Portfolio Identity

```ts
interface PortfolioIdentity {
  portfolioId: string;
  canonicalName: string;
  revision: string;
  assetRefs: AssetRef[];
}
```

Assets retain their CALP identities.

PGP does not rewrite or duplicate them.

---

## 6. No Universal Product Score

PGP MUST NOT treat a single scalar score as portfolio authority.

Invalid governing model:

```text
product_score = 83.7
```

Instead, PGP uses typed profiles.

```ts
interface PortfolioProfile {
  assetStrength: Profile;
  marketEvidence: Profile;
  economics: Profile;
  autonomy: Profile;
  reuse: Profile;
  strategicValue: Profile;
  risk: Profile;
}
```

Example:

```text
Context Compiler

Asset strength       HIGH
Market evidence      LOW
Economics            UNKNOWN
Autonomy potential   HIGH
Reuse leverage       HIGH
Strategic value      HIGH
Risk                 LOW / MEDIUM
```

A ranking score MAY exist as a convenience projection.

It MUST NOT silently become disposition authority.

---

## 7. Profile Dimensions

### Asset Strength

Typical evidence:

```text
semantic stability
reuse
separability
implementation maturity
evidence maturity
ownership clarity
dependency stability
```

### Market Evidence

```text
buyer clarity
problem severity
external demand
willingness to pay
conversion
renewal
competitive substitution
```

### Economics

```text
revenue
gross margin
marginal cost
support cost
sales cost
renewal probability
revenue concentration
```

### Autonomy

```text
agent-operable work
human escalation frequency
customization burden
operational fragility
provider dependence
durable-effect frequency
```

### Reuse

```text
number of independent downstream products
cross-project reuse
generalization evidence
shared infrastructure leverage
```

### Strategic Value

```text
protocol validation
research value
option value
portfolio leverage
defensibility
dependency centrality
```

### Risk

```text
legal
security
privacy
operational
provider
reputational
financial
concentration
```

No one profile dominates automatically.

---

## 8. Human Attention as a First-Class Resource

PGP treats required human attention as a finite portfolio resource.

```ts
interface AttentionLedger {
  humanHours: number;
  interventions: number;
  escalationCount: number;
  supportEvents: number;
  strategicDecisions: number;
}
```

Possible derived metric:

```text
Attention Yield =
Annual Gross Profit
───────────────────
Human Hours Required
```

This is evidence.

It is not authority.

A high-attention asset may still be strategically important.

A low-attention asset may still be economically irrelevant.

---

## 9. Portfolio Dispositions

Canonical dispositions:

```text
EXPLORE
PROVE
PILOT
INVEST
SCALE
MAINTAIN
REPRICE
CONSTRAIN
COMBINE
LICENSE
OPEN
DORMANT
RETIRE
```

### EXPLORE

Resolve major semantic, technical, market, or ownership uncertainty.

### PROVE

Acquire stronger evidence for a material claim.

### PILOT

Test a bounded external commercial hypothesis.

### INVEST

Allocate additional capital, compute, or human attention.

### SCALE

Expand a demonstrated operating surface.

### MAINTAIN

Preserve the current commercial and operational boundary.

### REPRICE

Review economic terms without changing semantic standing.

### CONSTRAIN

Narrow scope, customer class, support boundary, or operating envelope.

### COMBINE

Evaluate whether two or more assets or surfaces should share a commercial boundary.

### LICENSE

Prefer bounded rights transfer over operating the asset directly.

### OPEN

Publish or open a previously restricted asset or implementation under authorized terms.

### DORMANT

Retain the asset with minimal operating work while preserving identity, evidence, and restart path.

### RETIRE

End active commercial operation while preserving historical evidence and successor relationships.

---

## 10. Disposition Record

```ts
interface PortfolioDisposition {
  assetRef: AssetRef;
  disposition: Disposition;

  evidenceRefs: EvidenceRef[];
  profile: PortfolioProfile;

  rationale: string;

  authorityRef: AuthorityRef;
  reviewTrigger?: Trigger;
}
```

A disposition MUST identify:

```text
subject
evidence
profile
rationale
authority
effective state change
review condition
```

---

## 11. Disposition Authority

PGP distinguishes evidence from strategic commitment.

Typical authority classes:

```text
PortfolioObservationAuthority
PortfolioReviewAuthority
PortfolioAllocationAuthority
PricingReviewAuthority
InvestmentAuthority
ScaleAuthority
DormancyAuthority
RetirementAuthority
PublicationAuthority
```

An agent may prepare a recommendation without holding the authority required to enact it.

### No Authority by Coalition

Two or more agents cannot expand their aggregate authority merely by agreeing.

---

## 12. Review Triggers

PGP uses triggers to create review work.

Triggers do not directly establish conclusions.

Examples:

```text
same mechanism used by >=3 independent products
→ CREATE generalization-review candidate

same request from >=3 independent customers
→ CREATE expansion-review candidate

zero meaningful use for 90 days
→ CREATE dormancy-review candidate

support burden exceeds declared envelope
→ CREATE constrain/reprice review

gross margin below threshold
→ CREATE economic review

critical security issue
→ PREEMPT ordinary growth work

external evidence contradicts marketed claim
→ SUSPEND affected projection
→ CREATE qualification review
```

Threshold values MUST be declared in the project or portfolio profile.

They MUST NOT be treated as universal protocol law.

---

## 13. Work Frontier

PGP and CALP together generate eligible portfolio work.

IEPE executes the work.

```text
CALP + PGP state
       ↓
eligible work graph
       ↓
READY COMMERCIAL FRONTIER
       │
       ├── qualify candidate
       ├── run benchmark
       ├── prepare pilot
       ├── improve product
       ├── answer support issue
       ├── investigate churn
       ├── update documentation
       ├── review pricing
       └── prepare retirement analysis
       ↓
IEPE coordinator
       ↓
agents
```

A blocked asset does not block unrelated portfolio work.

Example:

```text
Asset A
blocked on ownership

Asset B
needs benchmark

Asset C
support fix ready

Asset D
pilot analysis ready
```

Eligible frontier:

```text
B
C
D
```

---

## 14. Portfolio Scheduling

PGP SHOULD expose scheduling facts rather than conversational failure.

Possible scheduling state:

```ts
interface PortfolioWorkItem {
  id: string;
  assetRef: AssetRef;

  state:
    | "ready"
    | "claimed"
    | "blocked"
    | "deferred"
    | "superseded"
    | "done";

  blockerRefs?: BlockerRef[];
  authorityRefs: AuthorityRef[];
  evidenceRequirements: EvidenceRequirement[];
  budget?: WorkBudget;
}
```

Blocked is a scheduling fact.

It is not a request for an agent to violate the blocker.

---

## 15. Economic Unit

Every operating commercial surface SHOULD expose its economic unit.

Examples:

```text
per API call
per seat
per repository
per compiled context
per customer
per license
per publication
per month
per qualified output
```

Economic evidence SHOULD be attributable to the relevant surface rather than merged into portfolio-wide averages when attribution is possible.

---

## 16. Attention Economics

For a portfolio intended to generate low-touch compounding income, PGP SHOULD explicitly retain:

```text
revenue
gross margin
human hours
human interventions
support events
custom work
agent-resolved work
escalation frequency
```

Example:

```text
Annual Gross Profit:      $152,880
Human Attention:          62 hours/year

Attention Yield:          ~$2,466/hour
```

This may support a MAINTAIN or SCALE disposition.

It does not authorize one.

---

## 17. Portfolio Selection Heuristics

A three-profile minimum is recommended:

```text
ASSET PROFILE
Is this a real, reusable asset?

MARKET PROFILE
Is there evidence someone has reason to buy it?

AUTONOMY PROFILE
Can this operate without consuming disproportionate human attention?
```

Typical interpretations:

```text
High asset + low market
→ retain / research / do not productize yet

High market + low asset
→ service opportunity, weak durable IP

High asset + high market + low autonomy
→ license / integration / constrained service

High asset + high market + high autonomy
→ strong productization candidate
```

These are heuristics, not mandatory outcomes.

---

## 18. Negative Results

PGP MUST preserve negative commercial evidence.

Examples:

```text
pilot produced no measurable delta
customer would not pay
support burden exceeded margin
market existed but distribution failed
asset was reusable but not commercially separable
product generated revenue but required excessive human attention
```

Negative evidence should influence future review.

It must not be erased merely because the asset is later revisited.

---

## 19. Dormancy

Dormancy is a valid successful disposition.

A dormant asset:

```text
retains identity
retains evidence
retains ownership/protection state
retains successor/dependency relationships
retains restart conditions
```

but receives no ordinary growth work.

Example:

```yaml
disposition: DORMANT
reason:
  - no external demand
  - high internal reuse not currently needed
review_trigger:
  - third-party demand
  - dependency reactivation
  - strategic portfolio change
```

---

## 20. Retirement

Retirement is distinct from deletion.

A retired surface MUST preserve:

```text
asset identity
commercial history
receipts
customer obligations
evidence
negative results
replacement/successor links
```

Retirement MAY stop:

```text
new sales
new deployments
growth work
routine feature development
```

Protected customer and contractual obligations remain governed until satisfied or lawfully terminated.

---

## 21. Cross-Asset Relations

PGP may represent:

```text
DEPENDS_ON
REUSES
SUBSTITUTES_FOR
COMPLEMENTS
SHARES_BUYER_WITH
SHARES_DISTRIBUTION_WITH
SHARES_INFRASTRUCTURE_WITH
GENERATES_DEMAND_FOR
CANNIBALIZES
REPLACED_BY
```

A relation must not be asserted merely because two assets are conceptually adjacent.

---

## 22. Portfolio Context Lock

A commercial operating environment SHOULD compile a bounded portfolio context.

```json
{
  "portfolio": "chroma",

  "protocols": {
    "calp": "<pinned>",
    "pgp": "0.1.0",
    "iepe": "<pinned>",
    "icp": "<pinned>",
    "eqp": "<pinned>",
    "ses": "<pinned>",
    "pdp": "<pinned>"
  },

  "authorityResolution": {},
  "assets": [],
  "commercialSurfaces": [],
  "portfolioProfiles": [],
  "evidenceSnapshot": [],
  "openContradictions": [],
  "negativeResults": [],
  "activePilots": [],
  "operatingProducts": [],
  "dispositions": [],

  "budgets": {
    "capital": {},
    "humanAttention": {}
  }
}
```

Generated portfolio context is not normative authority.

Canonical sources must be corrected and recompiled.

---

## 23. Conformance Requirements

### P-01 Scalar Score Cannot Authorize Disposition

A score may rank candidates.

It may not itself enact:

```text
INVEST
SCALE
RETIRE
```

### P-02 Trigger Creates Review, Not Truth

Given:

```text
same request >= configured threshold
```

Valid:

```text
CREATE expansion review
```

Invalid:

```text
IMPLEMENT expansion
```

### P-03 Blocked Asset Does Not Freeze Portfolio

One blocked asset MUST NOT prevent unrelated eligible work from entering the Ready frontier.

### P-04 Attention Is Evidence

Attention Yield may support a disposition.

It does not authorize a disposition.

### P-05 Dormancy Preserves State

DORMANT MUST NOT erase semantic identity, evidence, ownership, or restart conditions.

### P-06 Retirement Preserves History

RETIRE MUST NOT destroy receipts or historical commercial evidence.

---

## 24. Protocol Manifest

Illustrative only; exact imported versions must be pinned in implementation.

```yaml
protocol:
  id: portfolio-governance
  short: PGP
  version: 0.1.0

authority:
  normative:
    - PROTOCOL.md
    - TERMINOLOGY.md
    - schemas/

imports:
  calp:
    bindings:
      AssetIdentity: exact
      AssetState: exact
      CommercialSurface: exact

  iepe:
    bindings:
      IssueContract: exact
      WorkFrontier: constrained
      Promotion: constrained

  eqp:
    bindings:
      Evidence: exact
      Qualification: exact

  pdp:
    bindings:
      DecisionAuthority: constrained

  icp:
    bindings:
      CommercialInteraction: constrained

  ses:
    bindings:
      Projection: constrained

exports:
  - PortfolioIdentity
  - PortfolioProfile
  - AttentionLedger
  - PortfolioDisposition
  - PortfolioTrigger
  - PortfolioWorkFrontier
```

---

## 25. Recommended Repository Shape

```text
portfolio-governance/
├── PROTOCOL.md
├── TERMINOLOGY.md
├── protocol-manifest.json
│
├── schemas/
│   ├── portfolio.schema.json
│   ├── portfolio-profile.schema.json
│   ├── attention-ledger.schema.json
│   ├── disposition.schema.json
│   └── trigger.schema.json
│
├── profiles/
│   ├── commercial-portfolio.profile.json
│   └── attention-budget.profile.json
│
└── conformance/
    ├── score-cannot-self-authorize.json
    ├── trigger-creates-review-not-promotion.json
    ├── blocked-asset-does-not-block-frontier.json
    ├── attention-is-evidence-not-authority.json
    ├── dormancy-preserves-state.json
    └── retirement-preserves-history.json
```

---

## 26. PGP Invariants

1. No universal scalar score has portfolio authority.
2. Human attention is a first-class resource.
3. A trigger creates review work, not a conclusion.
4. A blocked asset does not freeze unrelated work.
5. Portfolio disposition requires evidence, rationale, and authority.
6. Dormancy and retirement are valid outcomes.
7. Low revenue alone does not establish low strategic value.
8. High revenue alone does not establish high strategic value.
9. Repeated reuse is evidence for generalization review, not proof of generality.
10. The system may recommend; protected commitments require authorized disposition.
11. Historical commercial evidence is append-only.
12. Generated portfolio context remains downstream from canonical source authority.

---

## 27. Worked Example — Context Compiler

CALP establishes:

```text
Context Compiler
semantic:       canonical
implementation: tested
evidence:       defended
commercial:     candidate
operational:    inactive
```

PGP evaluates:

```text
Asset strength       HIGH
Market evidence      LOW
Economics            UNKNOWN
Autonomy potential   HIGH
Reuse leverage       HIGH
Strategic value      HIGH
Risk                 LOW / MEDIUM
```

PGP disposition:

```text
EXPLORE
```

Required work:

```text
define buyer
test willingness to pay
measure integration burden
```

After a paid pilot:

```text
external customer use: observed
willingness to pay: observed
economic delta: defended in one environment
general market fit: not established
```

PGP may then move the asset to:

```text
PILOT
MAINTAIN
INVEST
```

depending on the new evidence and authorized decision.

Suppose the product later reaches:

```text
customers:            7
MRR:                  $14,000
agent-resolved cases: 38
human escalations:    3
human hours/month:    5.2
```

PGP now has stronger economics and autonomy evidence.

It still does not self-authorize SCALE.

A ScaleAuthority holder must enact that disposition.

If three independent customers later request automatic context recompilation:

```text
trigger:
same request >= 3
```

PGP creates:

```text
EXPANSION REVIEW
```

The system then evaluates whether the capability is:

```text
a specialization
a new asset
a new commercial surface
or merely a customer-specific request
```

before any implementation is authorized.

---

## 28. Success Condition

PGP succeeds when the project can continuously allocate work across a changing portfolio without:

- requiring the portfolio to live in one person's memory;
- treating every promising asset as a product;
- treating every blocked asset as a blocked company;
- allowing metrics to become hidden authority;
- erasing failed commercial hypotheses;
- confusing low attention with high strategic value;
- confusing revenue with durable value.

The target state is:

```text
NORMAL WORK
    ↓
QUALIFIED ASSETS
    ↓
PORTFOLIO STATE
    ↓
AUTHORIZED DISPOSITIONS
    ↓
AUTONOMOUS WORK FRONTIER
    ↓
AGENT EXECUTION
    ↓
REVENUE + USAGE + FAILURE EVIDENCE
    ↓
UPDATED PORTFOLIO STATE
```

The objective is not maximum product count.

It is a portfolio of owned, qualified intellectual infrastructure whose strongest members can generate recurring economic value with progressively less required human operating labor.

