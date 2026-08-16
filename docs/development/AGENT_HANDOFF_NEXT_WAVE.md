# Browser next-wave agent handoff

## Baseline and authority

- Repository: `stoltembergg-png/browser`
- Base ref: `main`
- Base SHA: `48dad26a05a87c503e14a56a4ad9147acfa638e9`
- Protected required check: `CI / Quality Gate`
- Current open PRs at selection time: none
- Do not use `CURRENT_STATE.md` as live status; it is stale. Re-query GitHub and pin every branch/head SHA.
- No secrets, tokens, signing keys, credentials, or connection strings.
- No direct push to `main`, no fake status, no reviewer impersonation, no `--admin`, no gate relaxation.

## Primary agent: two cards to implement now

### PR-049 — WPT harness and pinned manifest

Dependencies observed closed: `#16`, `#26`, `#44`.

Objective:

- Build a deterministic, offline WPT subset runner against the real browser integration boundary.
- Pin the WPT revision and fixture identity.
- Produce an artifact schema bound to repository, commit SHA, tested tree, engine revision, OS and result digest.
- Cover known pass and known fail fixtures.
- Treat network access, missing fixture, no-run, skipped, timeout and identity mismatch as explicit failure/NO_GO.

Out of scope:

- Full WPT conformance claims.
- Internet-dependent tests.
- Changing the required check or release policy.
- Claiming the current Tauri shell is already a real browser; inspect the production path first.

Required evidence:

- RED test for missing/mismatched/empty result evidence.
- GREEN deterministic offline pass/fail fixture run.
- Result identity bound to exact SHA/tree/engine/OS.
- CI check that does not silently convert skipped/no-run to pass.
- `git diff --check`, focused tests, security/quality gates and remote CI on the exact head.

### PR-051 — Platform input/accessibility contract

Dependencies observed closed: `#17`, `#26`.

Objective:

- Define the OS-specific input, scale, focus, keyboard and accessibility boundary.
- Preserve event identity and ordering; no silent normalized-event loss.
- Add per-OS contract/smoke coverage for the supported reference scope.
- Document unsupported paths with explicit rejection/degradation, not silent success.

Out of scope:

- Complete screen-reader feature set.
- Platform support claims beyond executed evidence.
- Packaging/signing work owned by `PR-052`/`PR-053`/`PR-054`.
- Tauri/Servo integration claims not exercised by the test.

Required evidence:

- Contract tests for scale, focus, keyboard and pointer/input identity.
- Per-OS runner evidence where the contract claims OS behavior.
- Negative tests for unsupported/invalid input and lost focus.
- No shared workflow/release-file edits with PR-049 unless absolutely required and documented.
- Exact head SHA CI verification.

## Other agent queue: two cards, dependency-gated

The other agent may perform read-only reconnaissance now, but must not implement or open these cards until `PR-051` is merged into the current `main` and its new base/head/CI state is revalidated.

### PR-053 — Linux packaging

Predecessors: `#18` and `#51`; `#51` is currently open and is the active blocker.

After PR-051 merges:

- Start from the new `origin/main`, never from the old baseline.
- Select and document the Linux artifact format and distro floor.
- Enable real Tauri bundling for the selected experimental format.
- Add clean install/launch/uninstall/profile-preservation smoke.
- Bind artifact, manifest and checksum to the tested commit/tree.
- Do not claim all Linux distributions.

### PR-052 — Windows packaging

Predecessors: `#18` and `#51`; `#51` is currently open and is the active blocker.

After PR-051 merges:

- Start from the new `origin/main` and re-query all dependency evidence.
- Add Windows package/installer launch and uninstall smoke for the declared OS/arch floor.
- Preserve profile data across install/update/uninstall tests where applicable.
- Keep signing secret setup out of the PR; only add the signing hook/interface.
- Do not claim production signing or broad Windows support without a protected canary.

Recommended order for the other agent: `PR-053` first as the reference-platform release path, then `PR-052` after the Linux slice and shared packaging contract are stable. If both are worked in parallel after PR-051, they must not edit the same packaging manifest or release workflow without combined-tree reconciliation.

## Shared stop conditions

Stop and report `BLOCKED` if:

- a dependency is not verified on the current `main` SHA;
- the real Tauri/core/Servo path is absent where the card claims it;
- a test is only textual, fake-engine-only, skipped, empty or network-dependent;
- Cargo/toolchain/build failure prevents local proof and no remote proof exists;
- a workflow or release check is missing, stale, skipped, cancelled or attached to another SHA;
- a proposed change would relax a required gate or create a fake status;
- scope expands into another card.

## Required handback format

Return:

1. card ID and branch/head SHA;
2. files changed and why;
3. implementation versus contract-only portions;
4. commands actually run with exit codes and exact failures;
5. CI run IDs/check identities and artifact paths/digests;
6. dependencies revalidated after any merge/rebase;
7. remaining blockers and the next safe card.

Do not report a PR as complete merely because a branch exists, a plan is written, or a historical check was green.
