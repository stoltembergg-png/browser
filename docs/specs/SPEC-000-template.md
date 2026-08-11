# SPEC-000 — specification and acceptance template

> Status: template. This file is not a product decision and cannot satisfy an ADR gate.

## Identity

- Stable plan/Issue ID:
- Owner:
- Milestone:
- Status: proposed | accepted | superseded
- Source SHA/tree:

## Context

What problem or contract requires a specification? Link the authoritative planning/architecture document.

## Objective

State one verifiable outcome. Avoid implementation-first wording.

## Scope

List the interfaces, states, artifacts and users included.

## Out of scope

List explicitly what this specification does not decide or implement.

## Invariants and boundaries

- authority/ownership:
- allowed inputs and outputs:
- lifecycle/state invariants:
- security/privacy boundary:
- failure and recovery behavior:

## Alternatives

| Alternative | Evidence | Benefit | Cost/risk | Reason accepted/rejected |
|---|---|---|---|---|
| | | | | |

## Acceptance criteria

- [ ] Each criterion is observable and linked to a test, fixture, artifact or review.
- [ ] Negative paths and invalid/stale evidence are covered.
- [ ] Criteria identify the target platform/matrix when applicable.
- [ ] No criterion claims an unimplemented security or compatibility property.

## Testing and evidence

- Unit/contract:
- Integration/E2E:
- Negative/security:
- Performance/recovery:
- Required artifact identity: repository, event, base/head/tree SHA, run/attempt, revision and digest.

## Rollback and supersession

Describe the specific rollback, migration, forward-fix or stop/last-known-good path. A new decision must preserve the history of this specification.

## Open questions

Unresolved architectural choices become blockers and must be answered by an ADR before implementation.

## References

- `PROJECT_PLAN.md`
- `ARCHITECTURE.md`
- `TESTING_STRATEGY.md`
- `SECURITY_MODEL.md`
- relevant ADRs and manifests
