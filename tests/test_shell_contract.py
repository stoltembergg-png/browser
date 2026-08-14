"""Frontend shell contract acceptance tests for PR-012.

Validates the shell UI mock:
  - Omnibox and tab bar components exist with correct ARIA roles.
  - Navigation buttons exist with accessible labels.
  - Frontend JS does not use a generic `invoke` bridge.
  - Typed commands match the browser_domain::ui::UiCommand schema.
  - Malformed/unknown events are handled without crashing.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

WORKSPACE = Path(__file__).parents[1]
FRONTEND = WORKSPACE / "apps" / "desktop" / "frontend" / "index.html"
FRONTEND_JS = WORKSPACE / "apps" / "desktop" / "frontend" / "app.js"
FRONTEND_CSS = WORKSPACE / "apps" / "desktop" / "frontend" / "styles.css"
TAURI_CONFIG = WORKSPACE / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
TAURI_CAPABILITY = WORKSPACE / "apps" / "desktop" / "src-tauri" / "capabilities" / "main.json"

# Known typed command types matching browser_domain::ui::UiCommand
KNOWN_COMMAND_TYPES = {
    "navigate",
    "reload",
    "go_back",
    "go_forward",
    "stop",
    "new_tab",
    "close_tab",
    "select_tab",
}

# Known typed event types matching browser_domain::ui::UiEvent
KNOWN_EVENT_TYPES = {
    "tab_created",
    "tab_closed",
    "tab_selected",
    "navigation_started",
    "navigation_committed",
    "navigation_finished",
    "navigation_failed",
    "navigation_cancelled",
    "title_changed",
    "command_rejected",
}


class ShellContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.assertTrue(FRONTEND.exists(), f"frontend missing at {FRONTEND}")
        self.assertTrue(FRONTEND_JS.exists(), f"frontend JS missing at {FRONTEND_JS}")
        self.assertTrue(FRONTEND_CSS.exists(), f"frontend CSS missing at {FRONTEND_CSS}")
        self.html = FRONTEND.read_text(encoding="utf-8")
        self.js = FRONTEND_JS.read_text(encoding="utf-8")
        self.css = FRONTEND_CSS.read_text(encoding="utf-8")

    def test_omnibox_component_exists(self) -> None:
        """The omnibox (address bar) must be present with an accessible label."""
        self.assertIn('id="omnibox"', self.html)
        self.assertIn('aria-label="Address bar"', self.html)
        self.assertIn('type="text"', self.html)

    def test_tab_bar_component_exists(self) -> None:
        """The tab bar must exist with a tablist ARIA role."""
        self.assertIn('id="tab-bar"', self.html)
        self.assertIn('role="tablist"', self.html)
        self.assertIn('aria-label="Browser tabs"', self.html)

    def test_tab_strip_has_accessible_panel_contract(self) -> None:
        """Each tab must identify the panel it controls and the panel must be labelled."""
        self.assertIn('id="tab-panel"', self.html)
        self.assertIn('role="tabpanel"', self.html)
        self.assertIn('setAttribute("aria-controls", "tab-panel")', self.js)
        self.assertIn('content.setAttribute("aria-labelledby", "tab-"', self.js)

    def test_tab_strip_uses_keyboard_navigation(self) -> None:
        """The tab strip must provide roving focus and arrow/Home/End handling."""
        self.assertIn('tabindex="0"', self.html)
        self.assertIn('tabindex', self.html)
        self.assertIn('event.key === "ArrowRight"', self.js)
        self.assertIn('event.key === "ArrowLeft"', self.js)
        self.assertIn('event.key === "Home"', self.js)
        self.assertIn('event.key === "End"', self.js)
        self.assertIn('.focus()', self.js)

    def test_tab_selection_uses_typed_command(self) -> None:
        """Selecting a tab must send the typed select_tab command, not only mutate DOM."""
        self.assertIn('{ type: "select_tab", target_tab_id: id }', self.js)
        self.assertIn('cmd.type === "select_tab"', self.js)

    def test_stale_tab_events_are_ignored(self) -> None:
        """Events for unknown tabs must not create or select stale UI state."""
        self.assertIn('const eventTabId = event.tab_id || envelope.tab_id;', self.js)
        self.assertIn('!tabs.has(eventTabId)', self.js)

    def test_tab_close_affordance_is_explicit(self) -> None:
        """Tab close controls must be separate labeled buttons, not text-only decoration."""
        self.assertIn('className = "tab-close"', self.js)
        self.assertIn('aria-label", "Close tab "', self.js)
        self.assertIn('min-width: 1.75rem', self.css)
        self.assertNotIn('closeBtn.appendChild(el)', self.js)

    def test_navigation_buttons_have_labels(self) -> None:
        """Each navigation button must have an accessible aria-label."""
        labels = re.findall(r'aria-label="([^"]+)"', self.html)
        self.assertIn("Go back", labels)
        self.assertIn("Go forward", labels)
        self.assertIn("Reload page", labels)
        self.assertIn("New tab", labels)

    def test_navigation_controls_have_typed_result_handlers(self) -> None:
        """Navigation controls must project every typed action into a result."""
        for command_type in ("go_back", "go_forward", "reload", "stop"):
            self.assertIn(f'cmd.type === "{command_type}"', self.js)
        self.assertIn('event: { type: "navigation_cancelled" }', self.js)
        self.assertIn('event: { type: "command_rejected"', self.js)

    def test_navigation_history_uses_a_cursor_per_tab(self) -> None:
        """Back/forward must move a tab cursor instead of duplicating history."""
        self.assertIn("historyIndex", self.js)
        self.assertIn("history: []", self.js)
        self.assertIn("tab.historyIndex -= 1", self.js)
        self.assertIn("tab.historyIndex += 1", self.js)
        self.assertIn("tab.history.push", self.js)

    def test_no_generic_invoke(self) -> None:
        """The frontend must not use a generic invoke bridge."""
        for source_name, source in (("index.html", self.html), ("app.js", self.js)):
            self.assertNotIn(
                "__TAURI_INTERNALS__",
                source,
                f"{source_name} references Tauri internal invoke bridge",
            )
            self.assertNotIn(
                ".invoke(",
                source,
                f"{source_name} uses a generic .invoke() call",
            )

    def test_tauri_csp_is_local_only_and_has_no_inline_execution(self) -> None:
        config = json.loads(TAURI_CONFIG.read_text(encoding="utf-8"))
        csp = config["app"]["security"]["csp"]
        for required in (
            "default-src 'self'",
            "script-src 'self'",
            "style-src 'self'",
            "connect-src 'none'",
            "object-src 'none'",
            "base-uri 'none'",
            "frame-ancestors 'none'",
            "form-action 'none'",
        ):
            self.assertIn(required, csp)
        self.assertNotIn("unsafe-inline", csp)
        self.assertNotIn("unsafe-eval", csp)
        self.assertNotIn("http://", csp)
        self.assertNotIn("https://", csp)
        self.assertNotIn("<style>", self.html)
        self.assertNotIn("<script>", self.html)

    def test_tauri_capability_is_window_scoped_and_denies_plugins(self) -> None:
        capability = json.loads(TAURI_CAPABILITY.read_text(encoding="utf-8"))
        self.assertEqual(capability["identifier"], "main-window")
        self.assertEqual(capability["windows"], ["main"])
        self.assertEqual(capability["permissions"], [])
        serialized = json.dumps(capability)
        for denied_prefix in ("fs:", "http:", "process:", "shell:", "sql:"):
            self.assertNotIn(denied_prefix, serialized)

    def test_tauri_remote_navigation_is_not_configured(self) -> None:
        config = json.loads(TAURI_CONFIG.read_text(encoding="utf-8"))
        self.assertNotIn("devUrl", config.get("build", {}))
        self.assertNotIn("url", config["app"]["windows"][0])
        self.assertNotIn("<iframe", self.html)

    def test_status_region_is_accessible(self) -> None:
        """An aria-live status region must exist for screen reader announcements."""
        self.assertIn('aria-live="polite"', self.html)
        self.assertIn('role="status"', self.html)

    def test_typed_command_names_match_schema(self) -> None:
        """All command type strings in the JS must be known schema variants."""
        # Find all "type":"<value>" occurrences in command contexts.
        types = re.findall(r'"type":\s*"(\w+)"', self.js)
        for t in types:
            # Allow event types too (they use the same tag pattern)
            self.assertTrue(
                t in KNOWN_COMMAND_TYPES or t in KNOWN_EVENT_TYPES,
                f"Unknown type '{t}' in frontend — not in command or event schema",
            )

    def test_unknown_event_handler_exists(self) -> None:
        """The renderEvent function must have a default/unknown case."""
        self.assertIn(
            "default:",
            self.js,
            "renderEvent must handle unknown events with a default case",
        )


class UiSchemaContractTests(unittest.TestCase):
    """Smoke-test the browser_domain::ui JSON schema by loading the Rust crate.

    These tests call `cargo test -p browser-domain` which runs the unit tests
    that validate malformed/unknown commands and events.
    """

    def test_browser_domain_tests_pass(self) -> None:
        """All browser-domain unit tests must pass (includes malformed/unknown)."""
        import subprocess

        result = subprocess.run(
            ["cargo", "test", "-p", "browser-domain", "--", "--quiet"],
            capture_output=True,
            text=True,
            cwd=WORKSPACE,
            timeout=120,
        )
        self.assertEqual(
            result.returncode,
            0,
            f"browser-domain tests failed:\n{result.stdout}\n{result.stderr}",
        )
        self.assertIn("test result: ok", result.stdout)


if __name__ == "__main__":
    unittest.main()
