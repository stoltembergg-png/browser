# PR-049 Offline WPT Runner Contract

## Status and boundary

This contract defines the deterministic offline harness for the first WPT
subset. It does not claim full WPT conformance and it does not turn the current
Tauri shell into a browser. The harness proves manifest, fixture, adapter
protocol and evidence handling; a result is browser compatibility evidence only
when the adapter launches the declared real browser integration.

The checked-in subset is:

- manifest: `tests/fixtures/wpt/manifest.json`
- pinned upstream WPT revision: `40f78009c81558c6ff89915cb2546c2fe3ef3b97`
- result schema: `docs/contracts/wpt-runner-result.schema.json`
- runner: `scripts/wpt_runner.py`

The revision is recorded as an immutable commit SHA. The runner never downloads
WPT, contacts the public network, or resolves a floating revision.

## Manifest contract

The manifest is JSON with `schema_version: 1`, `adapter_protocol: 1`, a full
40-character lowercase `wpt_revision`, and a non-empty `tests` array. Each test
contains:

- unique `id`;
- relative local `path`, which must remain inside the manifest directory;
- `fixture_sha256`, the exact SHA-256 of the checked-in fixture;
- `expected`, either `pass` or `fail`;
- `owner`, `reason`, and ISO `recheck_after` when `expected` is `fail`.

A fixture is verified immediately before adapter invocation. Missing files,
path traversal, malformed hashes and content identity mismatch are hard
failures.

## Adapter protocol

The command is supplied explicitly with `--adapter-command` and optional
repeated `--adapter-arg` values. The runner invokes it once per test as:

```text
<adapter-command> <adapter-args> --test-id <id> --fixture <absolute-local-path>
```

The adapter receives these environment variables:

- `WPT_OFFLINE=1`;
- `WPT_NETWORK=disabled`;
- `WPT_MANIFEST_REVISION=<pinned SHA>`;
- `WPT_TEST_ROOT=<local manifest directory>`.

The adapter must write exactly one JSON object to stdout and no other output:

```json
{"test_id":"navigation/local-url","status":"pass"}
```

`test_id` must match the invocation and `status` must be one of `pass`, `fail`,
`timeout`, `notrun`, or `error`. Extra fields, malformed JSON, a wrong test ID,
non-zero exit, missing executable or timeout become `error`/`NO_GO`; adapter
stdout and stderr are never copied into the evidence artifact.

The environment flag is a protocol requirement, not an OS sandbox. An adapter
claiming a browser result must enforce offline behavior in the browser process
it starts. A future adapter that cannot prove this remains `NO_GO`.

## Result semantics

For each test:

- expected `pass` + actual `pass` → `pass`;
- expected `fail` + actual `fail` → `expected-fail`;
- expected `fail` + actual `pass` → `unexpected-pass`;
- expected `pass` + actual `fail` → `fail`;
- `timeout`, `notrun` and `error` remain their own blocking outcomes.

Only `pass` and triaged `expected-fail` can produce top-level `status: pass`.
Any `fail`, `unexpected-pass`, `timeout`, `notrun` or `error` produces
`status: NO_GO` and a non-zero process exit. The result contains counts and a
`result_digest` over the canonical result object before the digest field is
added. The artifact itself must still be hashed by the publishing workflow.

## Invocation example

```bash
python3 scripts/wpt_runner.py \
  --manifest tests/fixtures/wpt/manifest.json \
  --output "$RUNNER_TEMP/wpt-result.json" \
  --adapter-command <real-local-browser-adapter> \
  --repository "$GITHUB_REPOSITORY" \
  --commit-sha "$GITHUB_SHA" \
  --tree-sha "$(git rev-parse HEAD^{tree})" \
  --engine-revision "$ENGINE_REVISION" \
  --os-and-arch "$(uname -s)-$(uname -m)"
```

The adapter command is intentionally not checked into this contract: using a
fixture adapter as CI compatibility evidence would be a false green. Until the
real integration exposes this protocol, the checked-in tests validate the
harness and its fail-closed boundary only.
