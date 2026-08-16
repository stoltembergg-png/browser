#!/usr/bin/env bash
#
# macOS package smoke test — PR-054
#
# Tests clean install, launch and uninstall of the Browser DMG package.
# Must be run on the reference platform (macOS 12+ x86_64/arm64).
#
# Usage:
#   ./scripts/macos_package_smoke.sh [path/to/Browser_0.1.0.dmg]
#
# If no DMG path is given, the script validates the smoke contract
# (checks that the tauri config and support matrix are consistent) without
# actually installing anything.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_CONF="$REPO_ROOT/apps/desktop/src-tauri/tauri.conf.json"

# ── Contract validation (always runs) ─────────────────────────────────────

echo "=== macOS package smoke — contract validation ==="

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

# Check .dmg target is declared
HAS_DMG=$(python3 -c "
import json
with open('$TAURI_CONF') as f:
    data = json.load(f)
targets = data.get('bundle', {}).get('targets', [])
print('dmg' in targets)
" 2>/dev/null || echo "False")

if [ "$HAS_DMG" != "True" ]; then
    echo "FAIL: bundle.targets does not include 'dmg'"
    exit 1
fi

# Check macOS DMG config is declared
DMG_CONFIG=$(python3 -c "
import json
with open('$TAURI_CONF') as f:
    data = json.load(f)
dmg = data.get('bundle', {}).get('macOS', {}).get('dmg', {})
print('applications' in dmg)
" 2>/dev/null || echo "False")

if [ "$DMG_CONFIG" != "True" ]; then
    echo "FAIL: bundle.macOS.dmg.applications must be declared"
    exit 1
fi

echo "PASS: tauri.conf.json bundle configuration is valid"
echo "PASS: dmg target is declared"
echo "PASS: macOS DMG config is declared"
echo "PASS: contract validation complete"

# ── Install/Launch/Uninstall (only when DMG is provided) ──────────────────

DMG_PATH="${1:-}"

if [ -z "$DMG_PATH" ]; then
    echo ""
    echo "=== No DMG path provided — contract-only smoke complete ==="
    echo "To run full smoke: $0 path/to/Browser_0.1.0.dmg"
    exit 0
fi

if [ ! -f "$DMG_PATH" ]; then
    echo "FAIL: DMG file not found at $DMG_PATH"
    exit 1
fi

echo ""
echo "=== Full install/launch/uninstall smoke ==="

# 1. Mount DMG
echo "Step 1: Mounting $DMG_PATH..."
MOUNT_OUTPUT=$(hdiutil attach "$DMG_PATH" -nobrowse -noverify -noautoopen 2>&1)
echo "$MOUNT_OUTPUT"
MOUNT_POINT=$(echo "$MOUNT_OUTPUT" | grep -E '^/dev/' | tail -1 | awk '{print $3}')
if [ -z "$MOUNT_POINT" ] || [ ! -d "$MOUNT_POINT" ]; then
    echo "FAIL: could not determine mount point"
    exit 1
fi
echo "PASS: mounted at $MOUNT_POINT"

# 2. Copy .app to /Applications (simulated install)
PRODUCT_NAME="Browser"
APP_PATH="$MOUNT_POINT/$PRODUCT_NAME.app"
if [ ! -d "$APP_PATH" ]; then
    echo "FAIL: $PRODUCT_NAME.app not found in DMG at $APP_PATH"
    hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
    exit 1
fi

echo "Step 2: Installing to /Applications..."
DEST_PATH="/Applications/$PRODUCT_NAME.app"
if [ -d "$DEST_PATH" ]; then
    rm -rf "$DEST_PATH"
fi
cp -R "$APP_PATH" "/Applications/"
if [ ! -d "$DEST_PATH" ]; then
    echo "FAIL: install directory not found at $DEST_PATH"
    hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
    exit 1
fi
echo "PASS: install"

# 3. Unmount DMG
hdiutil detach "$MOUNT_POINT" 2>/dev/null || true

# 4. Launch
echo "Step 3: Launching browser..."
open -a "$PRODUCT_NAME"
sleep 3
# Check if process is running
if pgrep -x "$PRODUCT_NAME" > /dev/null; then
    echo "PASS: launch (process running)"
    # Kill it
    pkill -x "$PRODUCT_NAME" 2>/dev/null || true
    sleep 1
else
    echo "FAIL: browser process not found after launch"
    rm -rf "$DEST_PATH"
    exit 1
fi

# 5. Uninstall
echo "Step 4: Uninstalling..."
if [ -d "$DEST_PATH" ]; then
    rm -rf "$DEST_PATH"
fi
if [ -d "$DEST_PATH" ]; then
    echo "FAIL: app still exists in /Applications after uninstall"
    exit 1
else
    echo "PASS: uninstall"
fi

echo ""
echo "=== All smoke steps passed ==="