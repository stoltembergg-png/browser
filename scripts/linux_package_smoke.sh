#!/usr/bin/env bash
#
# Linux package smoke test — PR-053
#
# Tests clean install, launch and uninstall of the Browser .deb package.
# Must be run on the reference platform (Ubuntu 24.04 LTS x86_64).
#
# Usage:
#   ./scripts/linux_package_smoke.sh [path/to/browser.deb]
#
# If no .deb path is given, the script validates the smoke contract
# (checks that the script and tauri config are consistent) without
# actually installing anything.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_CONF="$REPO_ROOT/apps/desktop/src-tauri/tauri.conf.json"

# ── Contract validation (always runs) ─────────────────────────────────────

echo "=== Linux package smoke — contract validation ==="

if [ ! -f "$TAURI_CONF" ]; then
    echo "FAIL: tauri.conf.json not found at $TAURI_CONF"
    exit 1
fi

# Check bundle.active is true using python3
BUNDLE_ACTIVE=$(python3 -c "
import json, sys
with open('$TAURI_CONF') as f:
    data = json.load(f)
print(data.get('bundle', {}).get('active', False))
" 2>/dev/null || echo "false")

if [ "$BUNDLE_ACTIVE" != "True" ]; then
    echo "FAIL: bundle.active is not true in tauri.conf.json"
    exit 1
fi

# Check .deb target is declared
HAS_DEB=$(python3 -c "
import json
with open('$TAURI_CONF') as f:
    data = json.load(f)
targets = data.get('bundle', {}).get('targets', [])
print('deb' in targets)
" 2>/dev/null || echo "False")

if [ "$HAS_DEB" != "True" ]; then
    echo "FAIL: bundle.targets does not include 'deb'"
    exit 1
fi

echo "PASS: tauri.conf.json bundle configuration is valid"
echo "PASS: .deb target is declared"
echo "PASS: contract validation complete"

# ── Install/Launch/Uninstall (only when .deb is provided) ────────────────

DEB_PATH="${1:-}"

if [ -z "$DEB_PATH" ]; then
    echo ""
    echo "=== No .deb path provided — contract-only smoke complete ==="
    echo "To run full smoke: $0 path/to/browser_0.1.0_amd64.deb"
    exit 0
fi

if [ ! -f "$DEB_PATH" ]; then
    echo "FAIL: .deb file not found at $DEB_PATH"
    exit 1
fi

echo ""
echo "=== Full install/launch/uninstall smoke ==="

# 1. Clean install
echo "Step 1: Installing $DEB_PATH..."
sudo dpkg -i "$DEB_PATH" 2>&1 || sudo apt-get install -f -y 2>&1
echo "PASS: install"

# 2. Launch (start and verify process)
echo "Step 2: Launching browser..."
browser &
BROWSER_PID=$!
sleep 3
if kill -0 "$BROWSER_PID" 2>/dev/null; then
    echo "PASS: launch (PID $BROWSER_PID)"
    kill "$BROWSER_PID" 2>/dev/null || true
    wait "$BROWSER_PID" 2>/dev/null || true
else
    echo "FAIL: browser process exited immediately"
    sudo dpkg -r browser 2>/dev/null || true
    exit 1
fi

# 3. Uninstall
echo "Step 3: Uninstalling..."
sudo dpkg -r browser 2>&1
if dpkg -s browser 2>/dev/null | grep -q "Status: install ok installed"; then
    echo "FAIL: package still installed after remove"
    exit 1
else
    echo "PASS: uninstall"
fi

echo ""
echo "=== All smoke steps passed ==="
