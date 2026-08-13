# Release Strategy — Skeleton

> **Status:** experimental (PR-018)
> **Related:** CI_CD_STRATEGY.md §11, ADR-003

## Objective

Produce non-published build artifacts with manifests and checksums. This is a skeleton — no signing, no auto-update, no publishing to release channels.

## Artifacts

| Artifact | OS | Format | Status |
|---|---|---|---|
| `browser-desktop-linux-x86_64` | Linux | AppImage / deb | experimental |
| `browser-desktop-windows-x86_64` | Windows | MSI / exe | future |
| `browser-desktop-macos-x86_64` | macOS | dmg / app | future |

Currently only Linux AppImage is targeted. Windows and macOS packaging are PR-052/053/054.

## Checksums

Every artifact produces a SHA-256 checksum file:
```
browser-desktop-linux-x86_64_0.1.0.AppImage
browser-desktop-linux-x86_64_0.1.0.AppImage.sha256
```

The checksum file format is:
```
<sha256-hex>  <filename>
```

## Manifest

A release manifest (`release-manifest.json`) records:
```json
{
  "version": "0.1.0",
  "commit": "<full-sha>",
  "artifacts": [
    {
      "name": "browser-desktop-linux-x86_64",
      "filename": "browser-desktop-linux-x86_64_0.1.0.AppImage",
      "sha256": "<hash>",
      "size": <bytes>,
      "os": "linux",
      "arch": "x86_64"
    }
  ]
}
```

## Clean-install smoke

For each format, the release workflow performs:
1. **Install:** Install the artifact on a clean runner.
2. **Launch:** Start the binary and verify it does not crash immediately.
3. **Uninstall:** Remove the artifact and verify no remnants.

Currently, clean-install smoke is deferred until Tauri builds successfully in CI (PR-051+). The skeleton provides the workflow structure.

## Gating

- Release artifacts are NOT published to release channels.
- No signing keys are used.
- No auto-update endpoints.
- The workflow runs on `workflow_dispatch` (manual trigger) only.
- Artifacts are uploaded as GitHub Actions artifacts (temporary, deleted after 90 days).

## Future expansion

| Feature | PR |
|---|---|
| Windows packaging | PR-052 |
| Linux packaging | PR-053 |
| macOS packaging | PR-054 |
| Signing | PR-059 |
| Auto-update channels | PR-060 |
| Stable 1.0 gate | PR-063 |
