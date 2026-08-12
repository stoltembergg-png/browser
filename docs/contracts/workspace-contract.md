# Workspace contract — PR-004

> Status: implemented M0 bootstrap. This workspace contains only compilable package scaffolding; it does not implement browser behavior.

## Objective

Define the smallest compilable workspace that later PR-004 implementation must create, without importing Servo/Tauri behavior into the first bootstrap.

## Planned packages

| Package | Role | Allowed dependencies | Forbidden dependencies in M0 |
|---|---|---|---|
| `browser-domain` | IDs, value objects and pure domain contracts | std/serde only when approved | Tauri, Servo, platform, filesystem/network |
| `browser-core` | state owner/actor boundary, initially empty of product behavior | `browser-domain`, approved runtime primitives | Servo types, UI, direct platform |
| `engine-api` | internal minimal lifecycle/navigation/frame/input/shutdown contract | domain/value types only | public plugin registry, Servo types, UI |
| `test-support` | dev-only fixtures/fake support | workspace packages as test dependencies | production dependency path |
| `xtask` | validation/checker tooling | metadata/parser dependencies with policy | runtime/browser behavior |

## Invariants

- Cargo package names and dependency edges must agree with `docs/architecture-graph.yaml`.
- The implemented M0 workspace has exactly five members: `browser-domain`, `browser-core`, `engine-api`, `test-support` and `xtask`.
- `browser-core` must not depend on `servo-engine` or Servo types.
- `engine-api` is an internal SPI, not a public plugin/registry API.
- `test-support` cannot be included in release/runtime dependency closure.
- Workspace resolver, edition and MSRV are selected by the accepted toolchain policy, not guessed in this contract.
- No `unsafe`, `unwrap`, `expect` or `panic!` in domain/core/security/storage without the policy/justification required by the plan.

## Acceptance evidence

- `cargo metadata --locked` identifies exactly the intended packages. **PASS** on the implementation SHA.
- `cargo check --workspace` passes from a clean checkout. **PASS** on the implementation SHA.
- `cargo fmt --all -- --check` passes. **PASS** on the implementation SHA.
- `cargo test --workspace` passes with four unit tests. **PASS** on the implementation SHA.
- architecture validator agrees with package/edge manifest.
- forbidden-edge fixtures fail closed.
- package list and resolver are reproducible on the supported reference runner.

## Explicit non-goals

This contract does not choose the Servo revision, Tauri surface, process model, storage schema, HTTP/TLS policy, public engine API or release license. Those remain governed by their ADRs and later PRs.
