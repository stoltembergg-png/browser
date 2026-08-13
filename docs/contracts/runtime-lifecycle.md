# Runtime Lifecycle Contract

> **Status:** provisional (PR-015)
> **Related:** ADR-004, ARCHITECTURE.md §7

## States

```mermaid
stateDiagram-v2
  [*] --> Created
  Created --> Starting
  Starting --> Ready
  Starting --> Failed: startup error
  Ready --> Navigating
  Navigating --> Ready: committed/finished
  Navigating --> Failed: navigation error
  Ready --> Closing
  Navigating --> Closing
  Ready --> Suspended
  Suspended --> Ready: resume
  Suspended --> Closing: close
  Ready --> Crashed
  Navigating --> Crashed
  Failed --> Restarting: recovery policy permits
  Failed --> Closing: close
  Crashed --> Restarting: policy permits
  Crashed --> Exited: retry exhausted/user closes
  Restarting --> Ready: new generation
  Restarting --> Failed: retry exhausted
  Closing --> Exited
```

## Transitions

| From | To | Trigger | Side effects |
|---|---|---|---|
| Created | Starting | `EngineInstanceSpec` accepted | Resources allocated |
| Starting | Ready | `EngineReady` event | Engine accepts commands |
| Starting | Failed | startup error | `EngineCrashed` event; resources freed |
| Ready | Navigating | `Navigate` command | `NavigationStarted` event |
| Navigating | Ready | `NavigationFinished` | Tab state updated |
| Navigating | Failed | `NavigationFailed` | Error recorded |
| Ready | Closing | `Shutdown` command | Webviews dropped |
| Closing | Exited | event loop drained | `EngineExited` event |
| Ready | Crashed | engine panic | `EngineCrashed` event |
| Crashed | Restarting | policy permits | New generation created |
| Restarting | Ready | restart succeeds | `EngineReady` event |
| Restarting | Failed | retry exhausted | `EngineExited` event |

## Recovery and fencing (PR-028)

Crash/hang recovery is engine-neutral and does not claim process isolation. Each engine incarnation has a monotonic `EngineEpoch`; events and checkpoints from a previous epoch are rejected before changing browser state. Navigation generations remain fenced within the current epoch, so a stale replay after restart cannot commit into the new tab state.

Recovery records only redacted diagnostics (`[REDACTED]`) and classifies failures as panic, out-of-memory, or watchdog timeout. Restart attempts are bounded. A restart creates a new epoch and explicitly aborts any in-flight form submission; forms are never automatically resubmitted. Exhausted retries and abrupt shutdown produce a terminal result while retaining the last valid checkpoint for inspection/recovery policy.

Checkpoint writes use prepare → commit/abort semantics. The durable journal appends a complete record and calls `sync_all` before making it visible; recovery uses the newest complete record and ignores an incomplete final record. This is a persistence seam, not a process sandbox or a claim that the real Servo engine has passed the contract. Real engine crash/hang artifacts remain required by the engine-contract manifest.

## Terminal states

- `Exited`: engine instance is fully destroyed. Resources are freed. No further commands accepted.
- `Failed` with no restart: equivalent to terminal `Exited` with error.

## Non-allowed transitions

Any transition not listed above MUST fail closed:
- Receiving a `Navigate` while `Starting` → rejected with `NotSupported`
- Receiving `Shutdown` while `Crashed` → rejected with `NotSupported`
- Transition from `Exited` to any state → rejected with `NotSupported`

## Cancellation semantics

- `Stop` command cancels the current navigation: `NavigationCancelled` event emitted
- `Shutdown` cancels all pending commands: `CommandCancelled` events for each
- After `Exited`, all pending commands return `CommandCancelled`

## Versioning

- `ENGINE_API_VERSION = 1`
- Unknown versions rejected by both sides of the contract
- Breaking changes require version bump and superseding ADR
