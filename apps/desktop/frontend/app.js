      // UI shell contract mock — PR-012
      //
      // This script demonstrates the typed command/event schema without
      // connecting to a real Tauri bridge. Commands are sent via
      // `sendCommand` and events are rendered via `renderEvent`.
      // There is intentionally NO generic `invoke()` — every command
      // is a named, typed variant matching `browser_domain::ui::UiCommand`.
      //
      // In a real Tauri build the bridge layer calls
      // `browser_domain::ui::parse_command()` on the Rust side and
      // rejects malformed input before executing.
      "use strict";

      const UI_CONTRACT_VERSION = 1;
      let requestCounter = 0;
      let tabCounter = 0;
      let activeTabId = null;
      const tabs = new Map(); // tabId -> {id, title, url, loading, error, history}

      const omnibox = document.getElementById("omnibox");
      const tabBar = document.getElementById("tab-bar");
      const status = document.getElementById("status");
      const content = document.getElementById("tab-panel");

      // --- Command sending (mock — no generic invoke) ---

      function nextRequestId() {
        requestCounter++;
        return "req-" + requestCounter;
      }

      function sendCommand(command, tabId) {
        // Build a typed command envelope matching UiCommand schema.
        const envelope = {
          version: UI_CONTRACT_VERSION,
          request_id: nextRequestId(),
          tab_id: tabId || null,
          command: command,
        };
        // Mock: emit a synthetic event for demonstration.
        // Real Tauri: invoke a typed Tauri command, not a generic bridge.
        handleCommandResult(envelope);
      }

      function handleCommandResult(envelope) {
        // Simulate core processing and emitting an event back.
        const cmd = envelope.command;
        if (cmd.type === "navigate") {
          renderEvent({
            version: UI_CONTRACT_VERSION,
            tab_id: envelope.tab_id || activeTabId,
            event: { type: "navigation_started", url: cmd.url },
          });
        } else if (cmd.type === "new_tab") {
          const id = "tab-" + (++tabCounter);
          renderEvent({
            version: UI_CONTRACT_VERSION,
            tab_id: null,
            event: { type: "tab_created", tab_id: id },
          });
        } else if (cmd.type === "select_tab") {
          renderEvent({
            version: UI_CONTRACT_VERSION,
            tab_id: cmd.target_tab_id,
            event: { type: "tab_selected", tab_id: cmd.target_tab_id },
          });
        } else if (cmd.type === "close_tab") {
          renderEvent({
            version: UI_CONTRACT_VERSION,
            tab_id: cmd.target_tab_id,
            event: { type: "tab_closed", tab_id: cmd.target_tab_id },
          });
        }
      }

      // --- Event rendering (core → UI) ---

      function renderEvent(envelope) {
        const event = envelope.event;
        const eventTabId = event.tab_id || envelope.tab_id;
        if (event.type !== "tab_created" &&
            (!eventTabId || !tabs.has(eventTabId))) {
          status.textContent = "Ignored stale tab event";
          return;
        }
        switch (event.type) {
          case "tab_created": {
            tabs.set(event.tab_id, {
              id: event.tab_id,
              title: "New Tab",
              url: "",
              loading: false,
              error: null,
              canGoBack: false,
              canGoForward: false,
            });
            activeTabId = event.tab_id;
            content.setAttribute("aria-labelledby", "tab-" + event.tab_id);
            renderTabs();
            break;
          }
          case "tab_closed": {
            tabs.delete(event.tab_id);
            if (activeTabId === event.tab_id) {
              const remaining = Array.from(tabs.keys());
              activeTabId = remaining.length > 0 ? remaining[0] : null;
              if (activeTabId) {
                const fallbackTab = tabs.get(activeTabId);
                omnibox.value = fallbackTab.url || "";
                content.setAttribute("aria-labelledby", "tab-" + activeTabId);
              } else {
                content.removeAttribute("aria-labelledby");
                omnibox.value = "";
              }
            }
            break;
          }
          case "tab_selected": {
            if (!tabs.has(event.tab_id)) {
              status.textContent = "Ignored stale tab event";
              return;
            }
            activeTabId = event.tab_id;
            const selectedTab = tabs.get(event.tab_id);
            omnibox.value = selectedTab.url || "";
            content.setAttribute("aria-labelledby", "tab-" + event.tab_id);
            break;
          }
          case "navigation_started": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.url = event.url;
              tab.loading = true;
              tab.error = null;
              if (typeof event.can_go_back === "boolean") tab.canGoBack = event.can_go_back;
              if (typeof event.can_go_forward === "boolean") tab.canGoForward = event.can_go_forward;
              if (activeTabId === tab.id) {
                omnibox.value = event.url;
              }
            }
            status.textContent = "Loading " + event.url;
            break;
          }
          case "navigation_committed": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.url = event.url;
              tab.error = null;
              if (activeTabId === tab.id) omnibox.value = event.url;
            }
            break;
          }
          case "navigation_finished": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.url = event.url;
              tab.loading = false;
              tab.error = null;
              if (activeTabId === tab.id) omnibox.value = event.url;
            }
            status.textContent = "Ready";
            break;
          }
          case "navigation_failed": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.loading = false;
              tab.error = event.reason;
            }
            status.textContent = "Error: " + event.reason;
            break;
          }
          case "navigation_cancelled": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.loading = false;
              tab.error = null;
            }
            status.textContent = "Navigation stopped";
            break;
          }
          case "title_changed": {
            const tab = tabs.get(envelope.tab_id);
            if (tab) tab.title = event.title;
            break;
          }
          case "command_rejected": {
            const tab = tabs.get(envelope.tab_id || activeTabId);
            if (tab) {
              tab.loading = false;
              tab.error = event.reason;
            }
            status.textContent = "Error: " + event.reason;
            break;
          }
          default:
            // Unknown event — do not crash, log to status.
            status.textContent = "Unknown event received";
            break;
        }
        renderTabs();
      }

      function updateNavigationControls() {
        const tab = activeTabId ? tabs.get(activeTabId) : null;
        const back = document.getElementById("btn-back");
        const forward = document.getElementById("btn-forward");
        const reload = document.getElementById("btn-reload");
        const stop = document.getElementById("btn-stop");
        back.disabled = !tab || !tab.canGoBack;
        forward.disabled = !tab || !tab.canGoForward;
        reload.disabled = !tab || !tab.url || tab.loading;
        stop.disabled = !tab || !tab.loading;
      }

      // --- DOM rendering ---

      function renderTabs() {
        tabBar.innerHTML = "";
        const tabIds = Array.from(tabs.keys());
        tabs.forEach((tab) => {
          const tabId = "tab-" + tab.id;
          const item = document.createElement("div");
          item.className = "tab-item" + (tab.id === activeTabId ? " active" : "");
          const el = document.createElement("button");
          el.id = tabId;
          el.className = "tab";
          el.type = "button";
          el.setAttribute("role", "tab");
          el.setAttribute("aria-selected", String(tab.id === activeTabId));
          el.setAttribute("aria-controls", "tab-panel");
          el.setAttribute("aria-label", tab.title);
          el.setAttribute("tabindex", tab.id === activeTabId || tabIds.length === 1 ? "0" : "-1");

          const label = document.createElement("span");
          label.className = "tab-label";
          label.textContent = tab.title;
          el.appendChild(label);

          const closeBtn = document.createElement("button");
          closeBtn.className = "tab-close";
          closeBtn.setAttribute("type", "button");
          closeBtn.setAttribute("aria-label", "Close tab " + tab.title);
          closeBtn.textContent = "✕";
          closeBtn.addEventListener("click", (event) => {
            event.stopPropagation();
            sendCommand(
              { type: "close_tab", target_tab_id: tab.id },
              tab.id,
            );
          });
          el.addEventListener("click", () => selectTab(tab.id));
          el.addEventListener("keydown", (event) => {
            const currentIndex = tabIds.indexOf(tab.id);
            let nextIndex = currentIndex;
            if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % tabIds.length;
            if (event.key === "ArrowLeft") nextIndex = (currentIndex - 1 + tabIds.length) % tabIds.length;
            if (event.key === "Home") nextIndex = 0;
            if (event.key === "End") nextIndex = tabIds.length - 1;
            if (nextIndex !== currentIndex) {
              event.preventDefault();
              const nextTabId = tabIds[nextIndex];
              selectTab(nextTabId);
              document.getElementById("tab-" + nextTabId)?.focus();
            }
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              selectTab(tab.id);
            }
          });
          item.appendChild(el);
          item.appendChild(closeBtn);
          tabBar.appendChild(item);
        });
        updateNavigationControls();
      }

      function selectTab(id) {
        if (!tabs.has(id)) {
          status.textContent = "Ignored stale tab selection";
          return;
        }
        sendCommand({ type: "select_tab", target_tab_id: id }, id);
      }

      // --- Event listeners ---

      omnibox.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          const url = omnibox.value.trim();
          if (url) {
            sendCommand({ type: "navigate", url: url }, activeTabId);
          }
        }
      });

      document.getElementById("btn-new-tab").addEventListener("click", () => {
        sendCommand({ type: "new_tab" }, null);
      });

      document.getElementById("btn-back").addEventListener("click", () => {
        sendCommand({ type: "go_back" }, activeTabId);
      });

      document.getElementById("btn-forward").addEventListener("click", () => {
        sendCommand({ type: "go_forward" }, activeTabId);
      });

      document.getElementById("btn-reload").addEventListener("click", () => {
        sendCommand({ type: "reload" }, activeTabId);
      });

      document.getElementById("btn-stop").addEventListener("click", () => {
        sendCommand({ type: "stop" }, activeTabId);
      });

      // Initialize with one tab.
      sendCommand({ type: "new_tab" }, null);
