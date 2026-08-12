# Pull Request

## Related Issue

- Issue: #
- Stable plan ID: `PR-`
- Milestone:

## Objective

## Context

## Dependencies

Depends on:
-

Blocked until:
-

Blocks:
-

## Scope

## Out of Scope

## Implementation Plan

## Files / Components Expected

## Testing Plan

List commands and real results. If code does not exist yet, state `PLANNING_ONLY` and validate the normative artifact instead.

## Security Considerations

## Acceptance Criteria

- [ ] Scope respected
- [ ] Unit tests added/updated when behavior exists
- [ ] Integration/E2E tests added when required
- [ ] `cargo fmt --check` passed when Rust exists
- [ ] `cargo clippy -- -D warnings` passed when Rust exists
- [ ] `cargo test --workspace` passed when Rust exists
- [ ] dependency/license/advisory checks passed or explicitly blocked
- [ ] security/negative checks passed when applicable
- [ ] documentation updated
- [ ] ADR updated/created when required; proposed ADR is not treated as authority
- [ ] no new warning
- [ ] no untracked TODO
- [ ] no secret added
- [ ] acceptance criteria satisfied
- [ ] evidence is bound to repository, event, base/head/tree SHA and artifact digest where applicable

## Documentation

## Rollback Strategy

Use a risk-specific rollback: documentation revert, forward-fix for migrations, stop/last-known-good for deploys or releases. Do not use a generic revert as the only safety plan.

## Risks

## CI Gates

- [ ] Quality Gate status is green for the current SHA
- [ ] No skipped/cancelled/neutral/stale/wrong-SHA check was treated as success
- [ ] Required checks were not configured unless the workflow exists and has passed canaries
- [ ] Control-plane state is recorded as `UNVERIFIED`, `OFF`, `SHADOW` or `ENFORCED`

## Agent ownership

- [ ] I checked `CURRENT_STATE.md` and `AGENTS.md`.
- [ ] This Issue is not already owned by another agent, or coordination is linked.
- [ ] This Draft PR will not be merged before its dependencies.
