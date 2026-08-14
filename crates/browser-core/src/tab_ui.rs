//! Integration seam from typed UI commands to the browser-core tab manager.

use browser_domain::ids::{EngineInstanceId, ProfileId, TabId, Url};
use browser_domain::tab::{Tab, TabVisibility};
use browser_domain::ui::{
    validate_navigate_url, CommandEnvelope, EventEnvelope, UiCommand, UiEvent, UI_CONTRACT_VERSION,
};
use engine_api::events::{EngineEvent, NavigationGeneration};
use std::fmt;

use crate::ipc_bridge::{IpcBridge, IpcError};
use crate::navigation_policy::{NavigationAction, NavigationPolicy};
use crate::tab_manager::{RoutedEngineEvent, TabManager, TabManagerError};

/// Errors produced while connecting a typed UI command to the tab manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorError {
    Ipc(IpcError),
    Manager(TabManagerError),
    InvalidPayload(String),
    UnsupportedCommand(&'static str),
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipc(error) => write!(formatter, "IPC rejected command: {error}"),
            Self::Manager(error) => write!(formatter, "tab manager rejected command: {error}"),
            Self::InvalidPayload(reason) => write!(formatter, "invalid UI payload: {reason}"),
            Self::UnsupportedCommand(command) => {
                write!(formatter, "unsupported UI command: {command}")
            }
        }
    }
}

impl std::error::Error for CoordinatorError {}

impl From<IpcError> for CoordinatorError {
    fn from(error: IpcError) -> Self {
        Self::Ipc(error)
    }
}

impl From<TabManagerError> for CoordinatorError {
    fn from(error: TabManagerError) -> Self {
        Self::Manager(error)
    }
}

/// Owns the command/event seam for the multi-tab shell.
///
/// This is intentionally engine-neutral. Real Tauri registration and a real
/// engine host remain separate adapters; this coordinator proves that a
/// validated UI action is applied to exactly one manager tab and emits an
/// event with the same identity.
pub struct TabUiCoordinator {
    bridge: IpcBridge,
    manager: TabManager,
    next_tab_number: u64,
    next_engine_number: u64,
    next_navigation_generation: u32,
}

impl TabUiCoordinator {
    pub fn new() -> Self {
        Self {
            bridge: IpcBridge::new(),
            manager: TabManager::new(),
            next_tab_number: 1,
            next_engine_number: 1,
            next_navigation_generation: 1,
        }
    }

    pub fn handle_command(&mut self, raw: &str) -> Result<Vec<EventEnvelope>, CoordinatorError> {
        let envelope: CommandEnvelope = self.bridge.validate_command(raw)?;
        match envelope.command {
            UiCommand::NewTab => self.create_tab(),
            UiCommand::CloseTab { target_tab_id } => self.close_tab(&target_tab_id),
            UiCommand::SelectTab { target_tab_id } => self.select_tab(&target_tab_id),
            UiCommand::Navigate { url } => {
                let tab_id = envelope.tab_id.ok_or_else(|| {
                    CoordinatorError::InvalidPayload("navigate requires tab_id".into())
                })?;
                self.navigate(&tab_id, url)
            }
            UiCommand::Reload | UiCommand::GoBack | UiCommand::GoForward | UiCommand::Stop => {
                Err(CoordinatorError::UnsupportedCommand(
                    "engine command handled by the engine-host adapter",
                ))
            }
            UiCommand::DownloadStart { .. }
            | UiCommand::DownloadCancel { .. }
            | UiCommand::DownloadRetry { .. } => Err(CoordinatorError::UnsupportedCommand(
                "download command handled by the download coordinator",
            )),
        }
    }

    pub fn handle_engine_event(
        &mut self,
        routed: RoutedEngineEvent,
    ) -> Result<Option<EventEnvelope>, CoordinatorError> {
        let tab_id = routed.tab_id().clone();
        let event = routed.event().clone();
        self.manager.route_event(&routed)?;
        Ok(engine_event_to_ui(&tab_id, &event))
    }

    pub fn active_tab(&self) -> Option<&TabId> {
        self.manager.active_tab()
    }

    pub fn tab(&self, tab_id: &TabId) -> Option<&Tab> {
        self.manager.tab(tab_id)
    }

    pub fn binding(&self, tab_id: &TabId) -> Option<&crate::tab_manager::TabBinding> {
        self.manager.binding(tab_id)
    }

    pub fn tab_count(&self) -> usize {
        self.manager.tab_count()
    }

    pub fn is_visible(&self, tab_id: &str) -> bool {
        self.manager
            .tab(&TabId::new(tab_id))
            .is_some_and(|tab| tab.visibility() == TabVisibility::Visible)
    }

    pub fn current_url(&self, tab_id: &str) -> Option<&str> {
        self.manager.tab(&TabId::new(tab_id)).and_then(|tab| {
            tab.pending_url()
                .or_else(|| tab.current_url())
                .map(Url::as_str)
        })
    }

    fn create_tab(&mut self) -> Result<Vec<EventEnvelope>, CoordinatorError> {
        let tab_id = TabId::new(format!("tab-{}", self.next_tab_number));
        self.next_tab_number += 1;
        let engine_id = EngineInstanceId::new(format!("engine-{}", self.next_engine_number));
        self.next_engine_number += 1;
        self.manager
            .create_tab(tab_id.clone(), ProfileId::new("profile-default"), engine_id)?;
        self.bridge.register_tab(&tab_id.0);
        self.manager.select_tab(&tab_id)?;
        Ok(vec![
            event(
                Some(tab_id.clone()),
                UiEvent::TabCreated {
                    tab_id: tab_id.clone(),
                },
            ),
            event(Some(tab_id.clone()), UiEvent::TabSelected { tab_id }),
        ])
    }

    fn select_tab(&mut self, tab_id: &TabId) -> Result<Vec<EventEnvelope>, CoordinatorError> {
        self.manager.select_tab(tab_id)?;
        Ok(vec![event(
            Some(tab_id.clone()),
            UiEvent::TabSelected {
                tab_id: tab_id.clone(),
            },
        )])
    }

    fn close_tab(&mut self, tab_id: &TabId) -> Result<Vec<EventEnvelope>, CoordinatorError> {
        let was_active = self.manager.active_tab() == Some(tab_id);
        self.manager.close_tab(tab_id)?;
        self.bridge.unregister_tab(&tab_id.0);
        let mut events = vec![event(
            Some(tab_id.clone()),
            UiEvent::TabClosed {
                tab_id: tab_id.clone(),
            },
        )];
        if was_active {
            if let Some(fallback) = self.manager.live_tab_ids().into_iter().next() {
                self.manager.select_tab(&fallback)?;
                events.push(event(
                    Some(fallback.clone()),
                    UiEvent::TabSelected { tab_id: fallback },
                ));
            }
        }
        Ok(events)
    }

    fn navigate(
        &mut self,
        tab_id: &TabId,
        url: String,
    ) -> Result<Vec<EventEnvelope>, CoordinatorError> {
        validate_navigate_url(&url).map_err(CoordinatorError::InvalidPayload)?;
        match NavigationPolicy::default().classify(&url) {
            NavigationAction::Allow => {}
            NavigationAction::Deny | NavigationAction::Confirm => {
                return Err(CoordinatorError::InvalidPayload(
                    "navigation denied by scheme policy".to_string(),
                ));
            }
        }
        let binding =
            self.manager
                .binding(tab_id)
                .cloned()
                .ok_or_else(|| TabManagerError::TabNotFound {
                    tab_id: tab_id.clone(),
                })?;
        let generation = NavigationGeneration(self.next_navigation_generation);
        self.next_navigation_generation += 1;
        self.manager.route_event(&RoutedEngineEvent::new(
            tab_id.clone(),
            binding,
            EngineEvent::NavigationStarted {
                url: url.clone(),
                generation,
            },
        ))?;
        Ok(vec![event(
            Some(tab_id.clone()),
            UiEvent::NavigationStarted { url },
        )])
    }
}

impl Default for TabUiCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

fn event(tab_id: Option<TabId>, ui_event: UiEvent) -> EventEnvelope {
    EventEnvelope {
        version: UI_CONTRACT_VERSION,
        tab_id,
        event: ui_event,
    }
}

fn engine_event_to_ui(tab_id: &TabId, engine_event: &EngineEvent) -> Option<EventEnvelope> {
    let ui_event = match engine_event {
        EngineEvent::NavigationStarted { url, .. } => {
            UiEvent::NavigationStarted { url: url.clone() }
        }
        EngineEvent::NavigationCommitted { url, .. } => {
            UiEvent::NavigationCommitted { url: url.clone() }
        }
        EngineEvent::NavigationFinished { url, .. } => {
            UiEvent::NavigationFinished { url: url.clone() }
        }
        EngineEvent::NavigationFailed { reason, .. } => UiEvent::NavigationFailed {
            reason: reason.clone(),
        },
        EngineEvent::TitleChanged { title } => UiEvent::TitleChanged {
            title: title.clone(),
        },
        _ => return None,
    };
    Some(event(Some(tab_id.clone()), ui_event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser_domain::ids::{EngineInstanceId, ProfileId, TabId};
    use browser_domain::ui::{
        CommandEnvelope, EventEnvelope, UiCommand, UiEvent, UI_CONTRACT_VERSION,
    };
    use engine_api::events::{EngineEvent, NavigationGeneration};

    fn raw(command: UiCommand, tab_id: Option<&str>, request_id: &str) -> String {
        serde_json::to_string(&CommandEnvelope {
            version: UI_CONTRACT_VERSION,
            request_id: browser_domain::ids::RequestId::new(request_id),
            tab_id: tab_id.map(TabId::new),
            command,
        })
        .expect("command serializes")
    }

    fn new_tab(request_id: &str) -> String {
        raw(UiCommand::NewTab, None, request_id)
    }

    fn create_two_tabs(coordinator: &mut TabUiCoordinator) {
        coordinator
            .handle_command(&new_tab("request-1"))
            .expect("first tab");
        coordinator
            .handle_command(&new_tab("request-2"))
            .expect("second tab");
    }

    #[test]
    fn new_tab_creates_manager_record_and_emits_created_then_selected() {
        let mut coordinator = TabUiCoordinator::new();

        let events = coordinator
            .handle_command(&new_tab("request-1"))
            .expect("new tab");

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].event,
            UiEvent::TabCreated {
                tab_id: TabId::new("tab-1")
            }
        );
        assert_eq!(
            events[1].event,
            UiEvent::TabSelected {
                tab_id: TabId::new("tab-1")
            }
        );
        assert_eq!(coordinator.active_tab(), Some(&TabId::new("tab-1")));
        assert!(coordinator.tab(&TabId::new("tab-1")).is_some());
        assert_eq!(coordinator.tab_count(), 1);
    }

    #[test]
    fn select_switches_visibility_and_emits_only_targeted_selection() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);

        let events = coordinator
            .handle_command(&raw(
                UiCommand::SelectTab {
                    target_tab_id: TabId::new("tab-1"),
                },
                Some("tab-1"),
                "request-3",
            ))
            .expect("select tab");

        assert_eq!(
            events,
            vec![EventEnvelope {
                version: UI_CONTRACT_VERSION,
                tab_id: Some(TabId::new("tab-1")),
                event: UiEvent::TabSelected {
                    tab_id: TabId::new("tab-1")
                },
            }]
        );
        assert_eq!(coordinator.active_tab(), Some(&TabId::new("tab-1")));
        assert!(coordinator.is_visible("tab-1"));
        assert!(!coordinator.is_visible("tab-2"));
    }

    #[test]
    fn close_active_tab_emits_close_and_selects_existing_fallback() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);

        let events = coordinator
            .handle_command(&raw(
                UiCommand::CloseTab {
                    target_tab_id: TabId::new("tab-2"),
                },
                Some("tab-2"),
                "request-3",
            ))
            .expect("close tab");

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].event,
            UiEvent::TabClosed {
                tab_id: TabId::new("tab-2")
            }
        );
        assert_eq!(
            events[1].event,
            UiEvent::TabSelected {
                tab_id: TabId::new("tab-1")
            }
        );
        assert_eq!(coordinator.active_tab(), Some(&TabId::new("tab-1")));
        assert_eq!(coordinator.tab_count(), 2);
        assert!(coordinator.tab(&TabId::new("tab-2")).is_some());
    }

    #[test]
    fn closing_non_active_tab_does_not_change_selection() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);
        let events = coordinator
            .handle_command(&raw(
                UiCommand::CloseTab {
                    target_tab_id: TabId::new("tab-1"),
                },
                Some("tab-1"),
                "request-3",
            ))
            .expect("close inactive tab");

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event,
            UiEvent::TabClosed {
                tab_id: TabId::new("tab-1")
            }
        );
        assert_eq!(coordinator.active_tab(), Some(&TabId::new("tab-2")));
    }

    #[test]
    fn close_target_must_match_envelope_scope() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);

        let result = coordinator.handle_command(&raw(
            UiCommand::CloseTab {
                target_tab_id: TabId::new("tab-2"),
            },
            Some("tab-1"),
            "request-3",
        ));

        assert!(matches!(
            result,
            Err(CoordinatorError::Ipc(IpcError::TargetMismatch { .. }))
        ));
        assert!(coordinator.active_tab().is_some());
        assert_eq!(coordinator.tab_count(), 2);
    }

    #[test]
    fn navigation_event_and_state_remain_scoped_to_one_tab() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);

        let events = coordinator
            .handle_command(&raw(
                UiCommand::Navigate {
                    url: "https://one.example".into(),
                },
                Some("tab-1"),
                "request-3",
            ))
            .expect("navigate tab one");

        assert_eq!(events[0].tab_id, Some(TabId::new("tab-1")));
        assert_eq!(
            events[0].event,
            UiEvent::NavigationStarted {
                url: "https://one.example".into(),
            }
        );
        assert_eq!(
            coordinator.current_url("tab-1"),
            Some("https://one.example")
        );
        assert_eq!(coordinator.current_url("tab-2"), None);
    }

    #[test]
    fn navigate_denies_disallowed_schemes() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);

        for url in [
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///etc/passwd",
        ] {
            let result = coordinator.handle_command(&raw(
                UiCommand::Navigate { url: url.into() },
                Some("tab-1"),
                format!("request-{url}").as_str(),
            ));
            assert!(
                matches!(result, Err(CoordinatorError::InvalidPayload(_))),
                "expected denial for {url:?}"
            );
        }
        assert_eq!(coordinator.current_url("tab-1"), None);
    }

    #[test]
    fn engine_event_with_other_tab_binding_is_rejected_without_mutation() {
        let mut coordinator = TabUiCoordinator::new();
        create_two_tabs(&mut coordinator);
        let tab_one = TabId::new("tab-1");
        let tab_two = TabId::new("tab-2");
        let binding_one = coordinator.binding(&tab_one).expect("binding").clone();

        let result = coordinator.handle_engine_event(RoutedEngineEvent::new(
            tab_two.clone(),
            binding_one,
            EngineEvent::NavigationStarted {
                url: "https://wrong.example".into(),
                generation: NavigationGeneration(1),
            },
        ));

        assert!(matches!(
            result,
            Err(CoordinatorError::Manager(
                TabManagerError::StaleBinding { .. }
            ))
        ));
        assert_eq!(coordinator.current_url("tab-1"), None);
        assert_eq!(coordinator.current_url("tab-2"), None);
    }

    #[test]
    fn close_command_roundtrips_with_explicit_target() {
        let command = UiCommand::CloseTab {
            target_tab_id: TabId::new("tab-9"),
        };
        let encoded = serde_json::to_string(&command).expect("serialize");
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, command);
    }

    #[allow(dead_code)]
    fn _type_contracts_are_reachable() {
        let _ = (EngineInstanceId::new("engine"), ProfileId::new("profile"));
    }
}
