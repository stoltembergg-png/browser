"""
macOS packaging smoke test for PR-054.

Validates that the Tauri bundle configuration enables macOS .dmg
packaging, that the DMG config is declared, and that a macOS
support matrix and smoke script exist.

This test does NOT build the actual .dmg (that requires a macOS
runner with Xcode). It validates the contract:
  1. bundle.targets includes "dmg" (macOS artifact format)
  2. macOS DMG applications config is declared
  3. A macOS support matrix document exists with OS floor
  4. An install/launch/uninstall smoke script exists
  5. No production signing secret is included in the PR

RED test: missing/broken configuration fails.
"""

import json
import pytest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TAURI_CONF = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
SUPPORT_MATRIX = REPO_ROOT / "docs" / "release" / "macos-support-matrix.md"
INSTALL_SCRIPT = REPO_ROOT / "scripts" / "macos_package_smoke.sh"


def test_tauri_conf_exists():
    """tauri.conf.json must exist and be valid JSON."""
    assert TAURI_CONF.exists(), f"Missing tauri config at {TAURI_CONF}"
    data = json.loads(TAURI_CONF.read_text())
    assert isinstance(data, dict), "tauri.conf.json must be a JSON object"


def test_bundle_includes_dmg_target():
    """bundle.targets must include 'dmg' as the macOS artifact format."""
    data = json.loads(TAURI_CONF.read_text())
    targets = data.get("bundle", {}).get("targets", [])
    assert "dmg" in targets, (
        f"bundle.targets must include 'dmg'; got {targets}. "
        "PR-054 selects .dmg as the macOS artifact format."
    )


def test_macos_dmg_config_declared():
    """macOS DMG applications config must be declared."""
    data = json.loads(TAURI_CONF.read_text())
    dmg = (
        data.get("bundle", {})
        .get("macOS", {})
        .get("dmg", {})
    )
    assert "applications" in dmg, (
        "bundle.macOS.dmg.applications must be declared; "
        "use an array with name and path for the Applications folder symlink."
    )


def test_support_matrix_document_exists():
    """A macOS support matrix document must exist with a declared OS floor."""
    assert SUPPORT_MATRIX.exists(), (
        f"Missing macOS support matrix at {SUPPORT_MATRIX}. "
        "PR-054 requires documenting the macOS OS/arch floor."
    )
    content = SUPPORT_MATRIX.read_text()
    assert "OS floor" in content or "os floor" in content.lower(), (
        "Support matrix must declare an OS floor section."
    )
    # Must reference both architectures
    assert "x86_64" in content and ("arm64" in content or "Apple Silicon" in content), (
        "Support matrix must reference both x86_64 and arm64 architectures."
    )


def test_install_smoke_script_exists():
    """An install/launch/uninstall smoke script must exist."""
    assert INSTALL_SCRIPT.exists(), (
        f"Missing macOS package smoke script at {INSTALL_SCRIPT}. "
        "PR-054 requires a clean install/launch/uninstall smoke."
    )
    content = INSTALL_SCRIPT.read_text()
    assert "install" in content.lower(), "Smoke script must test install."
    assert "uninstall" in content.lower() or "remove" in content.lower(), (
        "Smoke script must test uninstall/remove."
    )
    # Script must be executable-style (shebang)
    assert content.startswith("#!"), "Smoke script must have a shebang line."


def test_no_signing_secret_in_config():
    """No signing secret or certificate path should be in the config."""
    data = json.loads(TAURI_CONF.read_text())
    config_str = json.dumps(data)
    forbidden = ["password", "secret", "token", "private_key", "cert_path", "notarize"]
    for word in forbidden:
        assert word not in config_str.lower(), (
            f"bundle config must not contain '{word}'; "
            "signing/notarization secrets are out of scope for PR-054."
        )


def test_install_smoke_usage_matches_parameter():
    """The documented DMG argument must match the shell parameter."""
    content = INSTALL_SCRIPT.read_text()
    assert (
        "./scripts/macos_package_smoke.sh "
        "[path/to/Browser_0.1.0.dmg]"
    ) in content or (
        "macos_package_smoke.sh "
        "[path/to/Browser_0.1.0.dmg]"
    ) in content, "Smoke script usage must document DMG path parameter"
    assert "-DebPath" not in content, (
        "Smoke script usage must not advertise the unrelated -DebPath parameter"
    )
    assert "-InstallerPath" not in content, (
        "Smoke script usage must not advertise the unrelated -InstallerPath parameter"
    )