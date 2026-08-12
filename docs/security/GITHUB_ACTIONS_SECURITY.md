# GitHub Actions security policy

## Status

This policy is normative for workflows. The repository now contains the initial `ci-quality-gate.yml` workflow, but no required check has been configured yet. The operational control-plane remains `UNVERIFIED`; this document is not evidence that enforcement exists.

## Action pinning

- Critical workflows must use third-party Actions pinned to full commit SHA, not mutable tags or branches.
- The action repository, exact SHA, source/license, purpose, permissions and update owner must be documented.
- A tag or release name may appear only as a comment/reference beside the immutable SHA; it is never the trust anchor.
- Changing an action SHA is a security-sensitive PR owned by `.github/CODEOWNERS` with diff/release-note review.
- Local composite actions and scripts are reviewed as code and must not execute untrusted PR content with secrets.

## Token permissions

- Set workflow/job `permissions` explicitly and start from `contents: read` or `none`.
- Grant write permissions only to the smallest job and only for the required resource.
- Release/signing/provenance jobs must be isolated from untrusted pull-request execution.
- Do not store long-lived credentials in workflows. Prefer short-lived OIDC or environment-protected credentials when the release policy is ratified.
- Secrets are never exposed to forked pull requests or `pull_request_target` jobs that check out untrusted code.

## Event and fork policy

- Use `pull_request` for untrusted validation.
- `pull_request_target` is prohibited for workflows that execute PR code, install PR dependencies, or use secrets.
- Fork events must be safe with no secret access, no write token and no artifact publication.
- `workflow_run`, `workflow_call`, merge queue and release triggers require explicit trust-boundary review and identity binding.

## Inputs, scripts and artifacts

- Quote/sanitize all GitHub expression inputs before shell use; never interpolate issue/branch/PR text directly into commands.
- Pin toolchains and external downloads; verify checksums/signatures where available.
- Caches must not allow untrusted PRs to poison trusted release jobs.
- Artifacts must be immutable, retention-limited and bound to repository, event, base/head/tree SHA, run/attempt, workflow revision and digest.
- Release must reuse the exact artifact built and tested; rebuild by a later job is `NO_GO` unless identity is cryptographically verified.
- Attestation is insufficient alone: verify signer, repository, workflow, ref, digest, freshness and publication identity outside the runner.

## Workflow controls

Every workflow must declare:

- trigger and trust boundary;
- minimal permissions;
- concurrency and cancellation policy;
- job/step timeout;
- toolchain/action cache scope;
- matrix and runner assumptions;
- artifact retention and upload policy;
- failure behavior for missing, skipped, cancelled, neutral, stale or wrong-SHA evidence.

Versions must be reviewed for action pinning, runner image changes, dependency changes and secret scope. External Actions are allowed only after source, maintainer, license, security history and pin review.

## Rollout

1. `UNVERIFIED`: no authenticated evidence; no required checks.
2. `OFF`: bootstrap authenticated but enforcement intentionally disabled; negative canary required.
3. `SHADOW`: checks observe and report without merge authority; negative bypass/kill-switch/rollback canaries required.
4. `ENFORCED`: Ruleset and required checks reject missing/stale/wrong-identity evidence; revalidate continuously.

A transition cannot be inferred from YAML or a passing job alone. Failure returns the control-plane to the last verified safe state or `UNVERIFIED`.

## Release credentials

Release credentials, signing keys and attestations are not present in this repository. Any future credential change requires protected environment, human ownership, audit trail, rotation/revocation and a tested stop/last-known-good rollback. Never preserve a secret value in docs; use `[REDACTED]` when describing shape.
