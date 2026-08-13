# OS Support Matrix

> **Status:** provisional (PR-017)
> **Related:** ARCHITECTURE.md §7, ADR-003

## Objective

Document which operating systems the browser shell, adapter, and surface smoke support, and how CI validates each platform.

## CI Matrix

| OS | Runner | Scope | Status |
|---|---|---|---|
| Linux | `ubuntu-24.04` | Full quality gate (all crates + Tauri + Python tests) | ✅ primary |
| Windows | `windows-latest` | Core crates (browser-domain, engine-api, servo-engine) | ✅ matrix |
| macOS | `macos-latest` | Core crates (browser-domain, engine-api, servo-engine) | ✅ matrix |

## Gate semantics

- `fail-fast: false` — all OS runners execute even if one fails, so failures are visible.
- `quality-gate-aggregate` job requires BOTH `quality` and `os-matrix` to succeed.
- A runner failure is NOT a pass: `if: always()` with explicit status check.
- The aggregate job exits with error if any OS matrix job fails.

## Platform differences

### Linux (primary)
- Full workspace: `cargo check --workspace`, `cargo test --workspace`
- Tauri requires `libwebkit2gtk-4.1-dev` system package
- Python tests: documentation, security, quality-gate, shell bootstrap, shell contract
- Architecture graph validation via xtask

### Windows
- Core crates only: `browser-domain`, `engine-api`, `servo-engine`
- No WebKitGTK dependency (Tauri uses WebView2 on Windows)
- Rustfmt + clippy + tests on core crates
- Python tests not run (Python availability varies on Windows runners)

### macOS
- Core crates only: `browser-domain`, `engine-api`, `servo-engine`
- No WebKitGTK dependency (Tauri uses WKWebView on macOS)
- Rustfmt + clippy + tests on core crates
- Python tests not run (Python availability varies on macOS runners)

## Known limitations

1. **Tauri/browser-desktop not in matrix:** The `apps/desktop/src-tauri` crate requires WebKitGTK on Linux. Windows and macOS use different webview backends (WebView2, WKWebView) but the Tauri config is not yet tested cross-platform. Cross-platform Tauri validation is deferred to PR-051 (Platform input/accessibility contract).

2. **No artifact identity per OS:** The CI matrix validates compilation and tests, not binary artifacts. Binary artifact generation per OS is PR-052/053/054 (packaging).

3. **Input/scale/window differences:** These are deferred to PR-051. The current matrix validates that types compile and tests pass, not that visual rendering or input works.

4. **Python tests Linux-only:** The Python check scripts (`documentation_check.py`, `security_check.py`, `quality_gate_check.py`) and acceptance tests run only on Linux. Cross-platform Python validation is not needed for M1; the scripts validate workspace metadata, not platform-specific behavior.

## Reduction scope

If an OS fails and cannot be fixed:
- **Permitted:** Reduce the matrix to exclude that OS (e.g., remove `macos-latest` if macOS fails)
- **Forbidden:** Hide the failure or let a runner failure become a pass
- **Required:** Document the exclusion in this file with the reason

The aggregate gate enforces this: removing an OS from the matrix reduces the matrix job count, but the remaining jobs must all pass.
