# SPEC-048: Threat/abuse regression suite

## Purpose

Automated regression tests for threat model (THREAT_MODEL.md) scenarios.
Each test exercises a specific abuse vector and asserts that the browser
domain policy rejects it. Tests are **negative** — they prove that
dangerous inputs are blocked, not that legitimate inputs succeed.

## Scope

- Scheme injection (file, data, javascript, blob, vbscript)
- Popup storm (rate-limited denial)
- Credential-in-URL rejection
- Path traversal blocking
- Redirect abuse (redirect to denied scheme)
- Diagnostics PII leak prevention
- Privacy clearing completeness (no residual data)
- Permission denial by default (no implicit grant)

## Out of scope

- Positive/functional tests (covered by module unit tests)
- Performance/stress testing (PR-056)
- Fuzz harness (PR-056)

## Test location

Rust integration tests: `crates/browser-domain/tests/threat_abuse_regression.rs`
Python contract tests: `tests/test_threat_abuse_regression.py`

## Acceptance

All regression tests pass in CI. Any failure blocks merge.
