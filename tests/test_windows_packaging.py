"""
Windows packaging smoke test for PR-052.

Validates that the Tauri bundle configuration enables Windows .nsis
packaging, that the install mode is declared, and that a Windows
support matrix and smoke script exist.

This test does NOT build the actual .exe/.msi (that requires a Windows
runner with WebView2). It validates the contract:
  1. bundle.targets includes "nsis" (Windows installer format)
  2. Windows NSIS install mode is declared
  3. A Windows support matrix document exists
  4. An install/launch/uninstall smoke script exists
  5. No production signing secret is included in the PR

RED test: missing/broken configuration fails.
"""

import json
import pytest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TAURI_CONF = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
SUPPORT_MATRIX = REPO_ROOT / "docs" / "release" / "windows-support-matrix.md"
INSTALL_SCRIPT = REPO_ROOT / "scripts" / "windows_package_smoke.ps1"


def test_tauri_conf_exists():
    """tauri.conf.json must exist and be valid JSON."""
    assert TAURI_CONF.exists(), f"Missing tauri config at {TAURI_CONF}"
    data = json.loads(TAURI_CONF.read_text())
    assert isinstance(data, dict), "tauri.conf.json must be a JSON object"


def test_bundle_includes_nsis_target():
    """bundle.targets must include 'nsis' as the Windows installer format."""
    data = json.loads(TAURI_CONF.read_text())
    targets = data.get("bundle", {}).get("targets", [])
    assert "nsis" in targets, (
        f"bundle.targets must include 'nsis'; got {targets}. "
        "PR-052 selects NSIS as the Windows installer format."
    )


def test_windows_nsis_config_declared():
    """Windows NSIS install mode must be declared."""
    data = json.loads(TAURI_CONF.read_text())
    nsis = (
        data.get("bundle", {})
        .get("windows", {})
        .get("nsis", {})
    )
    assert "installMode" in nsis, (
        "bundle.windows.nsis.installMode must be declared; "
        "use 'perMachine' or 'currentUser'."
    )


def test_support_matrix_document_exists():
    """A Windows support matrix document must exist with a declared OS floor."""
    assert SUPPORT_MATRIX.exists(), (
        f"Missing Windows support matrix at {SUPPORT_MATRIX}. "
        "PR-052 requires documenting the Windows OS/arch floor."
    )
    content = SUPPORT_MATRIX.read_text()
    assert "WebView2" in content or "webview2" in content.lower(), (
        "Support matrix must reference WebView2 runtime dependency."
    )


def test_install_smoke_script_exists():
    """An install/launch/uninstall smoke script must exist."""
    assert INSTALL_SCRIPT.exists(), (
        f"Missing Windows package smoke script at {INSTALL_SCRIPT}. "
        "PR-052 requires a clean install/launch/uninstall smoke."
    )
    content = INSTALL_SCRIPT.read_text()
    assert "install" in content.lower(), "Smoke script must test install."
    assert "uninstall" in content.lower() or "remove" in content.lower(), (
        "Smoke script must test uninstall/remove."
    )


def test_install_smoke_usage_matches_parameter():
    """The documented installer argument must match the PowerShell parameter."""
    content = INSTALL_SCRIPT.read_text()
    assert (
        ".\\scripts\\windows_package_smoke.ps1 "
        "[-InstallerPath path\\to\\setup.exe]"
    ) in content, "Smoke script usage must document -InstallerPath"
    assert "-DebPath" not in content, (
        "Smoke script usage must not advertise the unrelated -DebPath parameter"
    )


def test_no_signing_secret_in_config():
    """No signing secret or certificate path should be in the config."""
    data = json.loads(TAURI_CONF.read_text())
    config_str = json.dumps(data)
    forbidden = ["password", "secret", "token", "private_key", "cert_path"]
    for word in forbidden:
        assert word not in config_str.lower(), (
            f"bundle config must not contain '{word}'; "
            "signing secrets are out of scope for PR-052."
        )
