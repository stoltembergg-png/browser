# Windows Support Matrix — PR-052

> **Status:** Experimental. The NSIS installer format is the selected Windows
> artifact for the reference platform. Production signing is out of scope; only
> the signing hook/interface is declared here.

## OS floor

The minimum supported Windows baseline is:

| Component | Minimum version |
|---|---|
| OS | Windows 10 (build 19041+) / Windows 11 |
| Architecture | x86_64 (amd64) |
| WebView2 | Evergreen runtime |
| .NET | Not required (Tauri uses WebView2) |

## Supported artifact format

- **Format:** NSIS installer (.exe)
- **Architecture:** `x86_64` (amd64)
- **Install mode:** perMachine (system-wide)
- **Target:** Tauri bundle with WebView2 backend

## Signing interface

- The signing hook is declared as an interface only.
- No production certificate, token, or secret is included in the PR.
- Actual signing requires CI secret setup outside this PR's scope.

## Not supported (no claim without evidence)

- MSI format (future work)
- ARM64 Windows
- Windows 8.1 and earlier
- Portable/zip distribution

## Smoke test

See `scripts/windows_package_smoke.ps1` for the clean
install/launch/uninstall smoke procedure.

The smoke test must be run on the reference platform (Windows 10/11 x86_64)
and must not be reported as passed without executed evidence on that
exact OS/arch combination.
