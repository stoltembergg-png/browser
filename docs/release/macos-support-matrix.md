# macOS Support Matrix — PR-054

> **Status:** Experimental. The `.dmg` format is the selected macOS
> artifact for the reference platform. Production signing/notarization is
> out of scope; only the signing hook/interface is declared here.

## OS floor

The minimum supported macOS baseline is:

| Component | Minimum version |
|---|---|
| OS | macOS 12 (Monterey) / macOS 13 (Ventura) / macOS 14 (Sonoma) / macOS 15 (Sequoia) |
| Architecture | x86_64 (Intel) and arm64 (Apple Silicon) |
| WebView | WebKit (system WebView via Tauri) |

## Supported artifact format

- **Format:** `.dmg` (Apple Disk Image)
- **Architectures:** `x86_64` and `arm64` (universal binary via Tauri)
- **Layout:** DMG with Applications folder symlink for drag-and-drop install

## Signing interface

- The signing/notarization hook is declared as an interface only.
- No production certificate, token, or secret is included in the PR.
- Actual signing/notarization requires CI secret setup outside this PR's scope.
- Hardened Runtime and App Sandbox configuration are declared in the bundle config.

## Not supported (no claim without evidence)

- `.app` bundle only distribution
- PKG installer (future work)
- macOS 11 (Big Sur) and earlier
- Rosetta 2-only execution claim

## Smoke test

See `scripts/macos_package_smoke.sh` for the clean
install/launch/uninstall smoke procedure.

The smoke test must be run on the reference platform (macOS 12+ Intel and Apple Silicon)
and must not be reported as passed without executed evidence on that
exact OS/arch combination.