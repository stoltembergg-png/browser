# Small-PR policy

## Contract

- One stable `PR-xxx` ID and one logical change per PR.
- Draft first; issue body and PR body must share Objective, Scope, Out of Scope, Dependencies, tests, acceptance, security, documentation and rollback.
- Do not bundle feature code with governance, workflow, migration, release or security-policy changes unless the plan explicitly makes them one unit.
- A PR may be prepared while a predecessor is open, but it must declare `Depends on` and `Blocked until`; it cannot merge early.
- Base/head/tree SHA and artifacts must be recorded for decisions and gates.
- A failed, skipped, stale or absent gate blocks merge; never weaken a check to make a PR green.

## Ownership

Use `status:ready`, `status:in-progress` and `status:blocked` labels. An agent assumes one Issue explicitly and releases it on handoff. Another agent must not duplicate work without coordination in the Issue.

## Exceptions

Only an ADR or explicit governance change may alter this policy. A workflow or bot cannot self-authorize an exception, and AI output is not approval evidence.
