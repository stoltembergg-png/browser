# Session recovery runbook — PR-036

> **Status:** implemented in `browser-core::session_lifecycle` for the local journal seam.
> This runbook does not claim cloud sync, full profile migration, or real Servo recovery.

## Scope

The session lifecycle owns the ordering around shutdown persistence and safe restore:

```text
Running
  → Quiescing       stop admitting work; cancel pending commands/downloads
  → Persisting      validate and stage one SessionRecord
  → Committed       flush the complete journal record

Persisting → Aborted → Quiescing (retry)
```

A session record is visible to restore only after its complete journal line has
been written and `sync_all` has succeeded. The journal is append-only. Recovery
selects the newest complete record and ignores an incomplete final record.

## Shutdown procedure

1. Call `begin_shutdown(PendingWork)`.
2. Confirm the returned `QuiesceReceipt`; no new work may be admitted after this
   point, and pending command/download counts are acknowledged as cancelled.
3. Build and validate a `SessionRecord` containing browser-owned state only.
4. Call `prepare_session(record)`; the staged record is not recoverable yet.
5. Call `commit_session(pending)`; only a successful result is a committed
   shutdown.
6. On an explicit cancellation, call `abort_session(pending)`. The previous
   committed snapshot remains the recovery source.

A failed commit transitions to `Aborted`, discards the in-memory pending record,
and preserves the previous committed record. Retry starts with a new quiesce
phase; it does not admit new work into the failed transaction.

## Restore procedure

1. Open the session lifecycle against the profile-local journal.
2. Call `restore()` and treat an absent record as a clean profile.
3. Materialize returned `RestoredTab` values as placeholders under the browser
   core's normal tab/profile policy.
4. Require an explicit navigation policy before loading any URL.

Restore never dispatches engine commands, replays forms, resumes downloads, or
replays pending UI commands. The returned disposition is always
`Placeholder` in this card.

## Failure handling

| Failure | Required result |
|---|---|
| Invalid schema, version, URL, active index, or oversized record | Reject the record; do not partially restore tabs |
| Process interruption before the final newline | Ignore the torn final record; retain the last complete snapshot |
| Journal open/write/flush failure | Enter `Aborted`; retain the last in-memory committed snapshot; do not claim success |
| New work after quiesce | Reject with an observable phase error |
| Retry after an aborted transaction | Start a fresh transaction; never reuse a stale pending token |

## Evidence

The focused card tests cover:

- quiesce and cancellation acknowledgement;
- atomic commit and phase transitions;
- placeholder restore without navigation replay;
- torn final record recovery;
- failed commit with last-snapshot retention;
- abort and retry semantics.

Run locally with:

```bash
cargo test -p browser-core --test session_lifecycle
```

## Boundaries and limitations

- The journal is a local persistence seam in the current workspace; a future
  storage backend may implement the same transaction semantics.
- Cloud sync is out of scope.
- Full profile migration and OS-specific storage policy remain separate work.
- This code is engine-neutral and does not prove Servo surface, process
  isolation, sandbox, site isolation, or production-browser claims.
