# Specifications

Specifications are versioned contracts for one Issue or PR. They are not implementation, authority or security evidence by themselves.

## Authoring rules

- Copy `SPEC-000-template.md` to a stable identifier tied to an Issue.
- Keep status `proposed` until the required ADR/owner accepts it.
- Link every acceptance criterion to an executable test, fixture, artifact or review record.
- Record repository, commit/tree, platform and evidence identity.
- Treat missing, skipped, stale or malformed evidence as a blocker.
- Never use a specification to claim sandbox, site isolation, compatibility or release readiness that has not been demonstrated.

## Authority

`PROJECT_PLAN.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `PR_PLAN.md`, ADRs and machine-readable manifests remain authoritative according to `docs/document-authority.yaml`. A specification may refine a card but may not silently override those sources.
