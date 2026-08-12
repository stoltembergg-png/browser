# GitHub label taxonomy

Labels are orthogonal dimensions. Apply at most one label from each dimension unless a card explicitly needs more than one area.

## Areas

`area:architecture`, `area:browser-core`, `area:servo`, `area:tauri`, `area:ui`, `area:security`, `area:ci`, `area:testing`, `area:release`, `area:platform`, `area:storage`, `area:network`, `area:docs`.

## Types

`type:foundation`, `type:feature`, `type:refactor`, `type:test`, `type:security`, `type:tooling`, `type:docs`.

## Priority and risk

Use exactly one `priority:p0`–`priority:p3` and one `risk:low`–`risk:critical` from the Issue card.

## Status

- `status:ready`: Definition of Ready satisfied and no unresolved predecessor.
- `status:in-progress`: one owner explicitly assumed the Issue.
- `status:blocked`: dependency, ADR, evidence or external bootstrap is unresolved.

## Rules

Do not use labels as a substitute for DAG edges, milestone assignment or acceptance criteria. The stable `PR-xxx` ID and the Issue body remain authoritative for traceability. Default GitHub labels are not used for this workflow unless an explicit policy maps them.
