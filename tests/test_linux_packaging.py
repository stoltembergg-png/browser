"""
Linux packaging smoke test for PR-053.

Validates that the Tauri bundle configuration enables Linux .deb packaging
and that the declared dependencies and distro floor are documented and consistent.

This test does NOT build the actual .deb (that requires webkit2gtk and a
full desktop runtime). It validates the contract:
  1. bundle.active is true
  2. bundle.targets includes "deb" (experimental Linux format)
  3. Linux deb dependencies are declared
  4. A support matrix document exists with a declared distro floor
  5. An install/launch/uninstall script exists

RED test: missing/broken configuration fails.
"""

import json
import os
import pytest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TAURI_CONF = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
SUPPORT_MATRIX = REPO_ROOT / "docs" / "release" / "linux-support-matrix.md"
INSTALL_SCRIPT = REPO_ROOT / "scripts" / "linux_package_smoke.sh"


def test_tauri_conf_exists():
    """tauri.conf.json must exist and be valid JSON."""
    assert TAURI_CONF.exists(), f"Missing tauri config at {TAURI_CONF}"
    data = json.loads(TAURI_CONF.read_text())
    assert isinstance(data, dict), "tauri.conf.json must be a JSON object"


def test_bundle_is_active():
    """bundle.active must be true to enable packaging."""
    data = json.loads(TAURI_CONF.read_text())
    bundle = data.get("bundle", {})
    assert bundle.get("active") is True, (
        "bundle.active must be true for Linux packaging; "
        "set it in tauri.conf.json"
    )


def test_bundle_includes_deb_target():
    """bundle.targets must include 'deb' as the experimental Linux format."""
    data = json.loads(TAURI_CONF.read_text())
    targets = data.get("bundle", {}).get("targets", [])
    assert "deb" in targets, (
        f"bundle.targets must include 'deb'; got {targets}. "
        "PR-053 selects .deb as the Linux artifact format."
    )


def test_linux_deb_dependencies_declared():
    """Linux .deb dependencies must be declared for the distro floor."""
    data = json.loads(TAURI_CONF.read_text())
    deb = (
        data.get("bundle", {})
        .get("linux", {})
        .get("deb", {})
    )
    depends = deb.get("depends", [])
    assert len(depends) > 0, (
        "bundle.linux.deb.depends must declare at least one runtime dependency; "
        "webkit2gtk and gtk3 are the minimum for Tauri on Linux."
    )
    # Must include webkit2gtk (the rendering engine for Tauri on Linux)
    has_webkit = any("webkit2gtk" in d for d in depends)
    assert has_webkit, (
        f"bundle.linux.deb.depends must include webkit2gtk; got {depends}"
    )


def test_support_matrix_document_exists():
    """A Linux support matrix document must exist with a declared distro floor."""
    assert SUPPORT_MATRIX.exists(), (
        f"Missing Linux support matrix at {SUPPORT_MATRIX}. "
        "PR-053 requires documenting the distro floor."
    )
    content = SUPPORT_MATRIX.read_text()
    assert "Distro floor" in content or "distro floor" in content.lower(), (
        "Support matrix must declare a distro floor section."
    )


def test_install_smoke_script_exists():
    """An install/launch/uninstall smoke script must exist."""
    assert INSTALL_SCRIPT.exists(), (
        f"Missing Linux package smoke script at {INSTALL_SCRIPT}. "
        "PR-053 requires a clean install/launch/uninstall smoke."
    )
    content = INSTALL_SCRIPT.read_text()
    assert "install" in content.lower(), "Smoke script must test install."
    assert "uninstall" in content.lower() or "remove" in content.lower(), (
        "Smoke script must test uninstall/remove."
    )
    # Script must be executable-style (shebang)
    assert content.startswith("#!"), "Smoke script must have a shebang line."
