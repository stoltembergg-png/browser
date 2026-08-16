"""Shell bootstrap acceptance tests for PR-011.

Validates that the Tauri shell:
  - Has a local frontend document with no remote URLs.
  - Has a restrictive CSP that blocks remote content.
  - Does not expose a generic invoke bridge, commands, or events.
  - The Tauri config points to the local frontend directory.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

WORKSPACE = Path(__file__).parents[1]

FRONTEND = WORKSPACE / "apps" / "desktop" / "frontend" / "index.html"
TAURI_CONF = WORKSPACE / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
TAURI_MAIN = WORKSPACE / "apps" / "desktop" / "src-tauri" / "src" / "main.rs"


def _urls_in_html(text: str) -> list[str]:
    """Return http/https URLs found in the HTML text."""
    import re

    return re.findall(r"https?://[^\s\"'<>]+", text)


class ShellBootstrapTests(unittest.TestCase):
    def setUp(self) -> None:
        self.assertTrue(FRONTEND.exists(), f"frontend missing at {FRONTEND}")
        self.assertTrue(TAURI_CONF.exists(), f"tauri.conf.json missing at {TAURI_CONF}")
        self.assertTrue(TAURI_MAIN.exists(), f"main.rs missing at {TAURI_MAIN}")
        self.frontend_text = FRONTEND.read_text(encoding="utf-8")
        self.conf = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
        self.main_text = TAURI_MAIN.read_text(encoding="utf-8")

    def test_frontend_has_no_remote_urls(self) -> None:
        """AC: no remote URL is loaded by the shell frontend."""
        urls = _urls_in_html(self.frontend_text)
        self.assertEqual(urls, [], f"frontend references remote URLs: {urls}")

    def test_csp_blocks_remote_content(self) -> None:
        """CSP must not allow http/https sources."""
        csp = self.conf.get("app", {}).get("security", {}).get("csp", "")
        self.assertTrue(csp, "CSP is missing")
        self.assertNotIn("http:", csp, "CSP allows http: sources")
        self.assertNotIn("https:", csp, "CSP allows https: sources")
        self.assertIn("default-src 'self'", csp, "CSP default-src is not 'self'")

    def test_no_generic_invoke_bridge(self) -> None:
        """The shell must not expose Tauri commands, events, or generic invoke."""
        self.assertNotIn(
            "invoke_handler",
            self.main_text,
            "main.rs registers an invoke handler",
        )
        self.assertNotIn(
            "#[tauri::command]",
            self.main_text,
            "main.rs registers Tauri commands",
        )
        self.assertNotIn(
            ".emit(",
            self.main_text,
            "main.rs emits Tauri events",
        )
        self.assertNotIn(
            ".listen(",
            self.main_text,
            "main.rs listens to Tauri events",
        )

    def test_frontend_dist_is_local(self) -> None:
        """frontendDist must point to the local frontend directory, not a URL."""
        build = self.conf.get("build", {})
        dist = build.get("frontendDist", "")
        self.assertTrue(dist, "frontendDist is not set")
        self.assertFalse(
            dist.startswith("http://") or dist.startswith("https://"),
            f"frontendDist is remote: {dist}",
        )

    def test_bundle_activation_is_explicit(self) -> None:
        """Bundle activation must be an explicit boolean configuration."""
        bundle = self.conf.get("bundle", {})
        self.assertIn("active", bundle, "bundle.active is not configured")
        self.assertIsInstance(
            bundle["active"],
            bool,
            "bundle.active must be a boolean",
        )

    def test_bundle_configuration(self) -> None:
        """Bundle must be either disabled (bootstrap) or have valid
        experimental targets (PR-053 Linux packaging).

        When bundle.active is true, the config must declare:
        - At least one target format (e.g. 'deb' for Linux)
        - Linux deb dependencies (webkit2gtk, gtk3)
        """
        bundle = self.conf.get("bundle", {})
        active = bundle.get("active", True)
        if not active:
            # Bootstrap milestone: bundling disabled — valid
            return
        # PR-053+: experimental bundling enabled
        targets = bundle.get("targets", [])
        self.assertGreater(
            len(targets), 0,
            "bundle.active is true but no targets declared",
        )
        linux_deb = (
            bundle.get("linux", {}).get("deb", {}).get("depends", [])
        )
        self.assertGreater(
            len(linux_deb), 0,
            "bundle.linux.deb.depends must declare runtime dependencies",
        )

    def test_window_is_resizable(self) -> None:
        """The window must be resizable per PR-011 scope."""
        windows = self.conf.get("app", {}).get("windows", [])
        self.assertTrue(len(windows) >= 1, "no window configured")
        self.assertTrue(
            windows[0].get("resizable", False),
            "window is not resizable",
        )


if __name__ == "__main__":
    unittest.main()
