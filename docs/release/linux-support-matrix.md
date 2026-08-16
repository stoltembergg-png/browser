# Linux Support Matrix — PR-053

> **Status:** Experimental. The `.deb` format is the selected Linux artifact
> for the reference platform. Broader distribution support is explicit future
> work and must not be claimed without executed evidence.

## Distro floor

The minimum supported Linux distribution baseline is:

| Component | Minimum version |
|---|---|
| Distro | Ubuntu 24.04 LTS (Noble Numbat) |
| Kernel | 6.6+ |
| glibc | 2.39+ |
| webkit2gtk | 4.1 |
| GTK | 3.24+ |

## Supported artifact format

- **Format:** `.deb` (Debian package)
- **Architecture:** `x86_64` (amd64)
- **Target:** Tauri bundle with webkit2gtk-4.1 backend

## Dependencies declared in bundle

- `libwebkit2gtk-4.1-0` — rendering engine
- `libgtk-3-0` — windowing toolkit
- `libayatana-appindicator3-1` — system tray support

## Not supported (no claim without evidence)

- AppImage, Flatpak, Snap (future work)
- ARM64/aarch64
- RPM-based distributions (Fedora, RHEL, openSUSE)
- Musl-based distributions (Alpine)

## Smoke test

See `scripts/linux_package_smoke.sh` for the clean
install/launch/uninstall smoke procedure.

The smoke test must be run on the reference platform (Ubuntu 24.04)
and must not be reported as passed without executed evidence on that
exact OS/arch combination.
