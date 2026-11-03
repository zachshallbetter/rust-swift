# ESP/RCP profile

This profile initializes a project as an unbound External State Protocol instance with an optional Recipient Contract Protocol record.

Use `.agents/bootstrap/scripts/esp-record.py <kind> --record '<json>'` to append a typed record. Existing identities are idempotent only when the content is identical; conflicting reuse is refused.

Initialization creates identity and empty append-only ledgers only. It does not invent an owner, recipient, counterparty, campaign, commitment, evidence, receipt, measurement, or learning result.

The external-state instance remains `unbound` and at lifecycle `bootstrap` until project authority supplies those records. RCP status is derived by `.agents/bootstrap/scripts/recipient-status.py` from `.agents/records/recipient.json` and an explicit as-of date.

Record ownership:

- `.agents/records/external-state.json`: ESP instance identity, lifecycle, authority bindings, and ledger paths.
- `.agents/records/campaigns.jsonl`: campaigns and external-state objectives.
- `.agents/records/commitments.jsonl`: counterparties, obligations, and closure.
- `.agents/records/evidence.jsonl`: evidence references and qualification lineage.
- `.agents/records/qualifications.jsonl`: EQP qualification dispositions.
- `.agents/records/receipts.jsonl`: observed external receipts.
- `.agents/records/measurements.jsonl`: reach, authority, adoption, propagation, and position measurements.
- `.agents/records/learning.jsonl`: retained learning and negative results.
- `.agents/records/recipient.json`: nullable RCP recipient obligation.
