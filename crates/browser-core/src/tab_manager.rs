//! Browser-core tab collection, engine binding, selection, and event routing.

use browser_domain::ids::{EngineInstanceId, NavigationId, ProfileId, TabId, TabTitle, Url};
use browser_domain::tab::{Tab, TabError, TabLifecycle};
use engine_api::events::{EngineEvent, NavigationGeneration};
use std::collections::HashMap;
use std::fmt;

/// Opaque binding between one domain tab and one engine incarnation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabBinding {
    tab_id: TabId,
    engine_instance_id: EngineInstanceId,
    generation: u64,
}

impl TabBinding {
    fn new(tab_id: TabId, engine_instance_id: EngineInstanceId, generation: u64) -> Self {
        Self {
            tab_id,
            engine_instance_id,
            generation,
        }
    }

    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    pub fn engine_instance_id(&self) -> &EngineInstanceId {
        &self.engine_instance_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// An engine event together with the binding under which it was observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedEngineEvent {
    tab_id: TabId,
    binding: TabBinding,
    event: EngineEvent,
}

impl RoutedEngineEvent {
    pub fn new(tab_id: TabId, binding: TabBinding, event: EngineEvent) -> Self {
        Self {
            tab_id,
            binding,
            event,
        }
    }

    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    pub fn binding(&self) -> &TabBinding {
        &self.binding
    }

    pub fn event(&self) -> &EngineEvent {
        &self.event
    }
}

/// Errors returned by tab collection and event routing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabManagerError {
    DuplicateTab { tab_id: TabId },
    TabNotFound { tab_id: TabId },
    ClosedTab { tab_id: TabId },
    StaleBinding { tab_id: TabId },
    TabMutation { tab_id: TabId, reason: String },
    AlreadyShutdown,
}

impl fmt::Display for TabManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTab { tab_id } => write!(formatter, "tab already exists: {tab_id}"),
            Self::TabNotFound { tab_id } => write!(formatter, "tab not found: {tab_id}"),
            Self::ClosedTab { tab_id } => write!(formatter, "tab is closed: {tab_id}"),
            Self::StaleBinding { tab_id } => {
                write!(formatter, "stale engine binding for tab: {tab_id}")
            }
            Self::TabMutation { tab_id, reason } => {
                write!(formatter, "tab {tab_id} mutation failed: {reason}")
            }
            Self::AlreadyShutdown => write!(formatter, "tab manager is already shut down"),
        }
    }
}

impl std::error::Error for TabManagerError {}

/// Owns all domain tabs and their current engine bindings.
#[derive(Debug, Default)]
pub struct TabManager {
    tabs: HashMap<TabId, Tab>,
    bindings: HashMap<TabId, TabBinding>,
    active_tab: Option<TabId>,
    next_binding_generation: u64,
    shutdown: bool,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            next_binding_generation: 1,
            ..Self::default()
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_tab(&self) -> Option<&TabId> {
        self.active_tab.as_ref()
    }

    pub fn tab(&self, tab_id: &TabId) -> Option<&Tab> {
        self.tabs.get(tab_id)
    }

    pub fn binding(&self, tab_id: &TabId) -> Option<&TabBinding> {
        self.bindings.get(tab_id)
    }

    pub fn live_tab_ids(&self) -> Vec<TabId> {
        self.tabs
            .iter()
            .filter_map(|(tab_id, tab)| {
                (tab.lifecycle() != TabLifecycle::Closed).then_some(tab_id.clone())
            })
            .collect()
    }

    pub fn create_tab(
        &mut self,
        tab_id: TabId,
        profile_id: ProfileId,
        engine_instance_id: EngineInstanceId,
    ) -> Result<TabBinding, TabManagerError> {
        if self.shutdown {
            return Err(TabManagerError::AlreadyShutdown);
        }
        if self.tabs.contains_key(&tab_id) {
            return Err(TabManagerError::DuplicateTab { tab_id });
        }
        let binding = TabBinding::new(
            tab_id.clone(),
            engine_instance_id.clone(),
            self.take_binding_generation(),
        );
        self.tabs.insert(
            tab_id.clone(),
            Tab::new(tab_id.clone(), profile_id, engine_instance_id),
        );
        self.bindings.insert(tab_id, binding.clone());
        Ok(binding)
    }

    pub fn rebind_engine(
        &mut self,
        tab_id: &TabId,
        engine_instance_id: EngineInstanceId,
    ) -> Result<TabBinding, TabManagerError> {
        self.ensure_live_tab(tab_id)?;
        let binding = TabBinding::new(
            tab_id.clone(),
            engine_instance_id,
            self.take_binding_generation(),
        );
        self.bindings.insert(tab_id.clone(), binding.clone());
        Ok(binding)
    }

    pub fn select_tab(&mut self, tab_id: &TabId) -> Result<(), TabManagerError> {
        self.ensure_live_tab(tab_id)?;
        if let Some(previous) = self.active_tab.clone() {
            if previous != *tab_id {
                let previous_tab =
                    self.tabs
                        .get_mut(&previous)
                        .ok_or_else(|| TabManagerError::TabNotFound {
                            tab_id: previous.clone(),
                        })?;
                previous_tab
                    .hide()
                    .map_err(|error| mutation_error(&previous, error))?;
            }
        }
        let selected = self
            .tabs
            .get_mut(tab_id)
            .ok_or_else(|| TabManagerError::TabNotFound {
                tab_id: tab_id.clone(),
            })?;
        selected
            .show()
            .map_err(|error| mutation_error(tab_id, error))?;
        selected
            .focus()
            .map_err(|error| mutation_error(tab_id, error))?;
        self.active_tab = Some(tab_id.clone());
        Ok(())
    }

    pub fn close_tab(&mut self, tab_id: &TabId) -> Result<(), TabManagerError> {
        self.ensure_live_tab(tab_id)?;
        let tab = self
            .tabs
            .get_mut(tab_id)
            .ok_or_else(|| TabManagerError::TabNotFound {
                tab_id: tab_id.clone(),
            })?;
        tab.begin_close()
            .map_err(|error| mutation_error(tab_id, error))?;
        tab.complete_close()
            .map_err(|error| mutation_error(tab_id, error))?;
        if self.active_tab.as_ref() == Some(tab_id) {
            self.active_tab = None;
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<usize, TabManagerError> {
        if self.shutdown {
            return Err(TabManagerError::AlreadyShutdown);
        }
        let live_tabs: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|(tab_id, tab)| {
                (tab.lifecycle() != TabLifecycle::Closed).then_some(tab_id.clone())
            })
            .collect();
        for tab_id in &live_tabs {
            self.close_tab(tab_id)?;
        }
        self.active_tab = None;
        self.shutdown = true;
        Ok(live_tabs.len())
    }

    pub fn route_event(&mut self, routed: &RoutedEngineEvent) -> Result<(), TabManagerError> {
        self.ensure_live_tab(&routed.tab_id)?;
        let current =
            self.bindings
                .get(&routed.tab_id)
                .ok_or_else(|| TabManagerError::TabNotFound {
                    tab_id: routed.tab_id.clone(),
                })?;
        if current != &routed.binding || routed.binding.tab_id != routed.tab_id {
            return Err(TabManagerError::StaleBinding {
                tab_id: routed.tab_id.clone(),
            });
        }

        let tab =
            self.tabs
                .get_mut(&routed.tab_id)
                .ok_or_else(|| TabManagerError::TabNotFound {
                    tab_id: routed.tab_id.clone(),
                })?;
        apply_event(tab, &routed.event).map_err(|error| mutation_error(&routed.tab_id, error))
    }

    fn ensure_live_tab(&self, tab_id: &TabId) -> Result<(), TabManagerError> {
        if self.shutdown {
            return Err(TabManagerError::AlreadyShutdown);
        }
        match self.tabs.get(tab_id) {
            None => Err(TabManagerError::TabNotFound {
                tab_id: tab_id.clone(),
            }),
            Some(tab) if tab.lifecycle() == TabLifecycle::Closed => {
                Err(TabManagerError::ClosedTab {
                    tab_id: tab_id.clone(),
                })
            }
            Some(_) => Ok(()),
        }
    }

    fn take_binding_generation(&mut self) -> u64 {
        let generation = self.next_binding_generation;
        self.next_binding_generation += 1;
        generation
    }
}

fn mutation_error(tab_id: &TabId, error: TabError) -> TabManagerError {
    TabManagerError::TabMutation {
        tab_id: tab_id.clone(),
        reason: error.to_string(),
    }
}

fn apply_event(tab: &mut Tab, event: &EngineEvent) -> Result<(), TabError> {
    match event {
        EngineEvent::EngineStarted | EngineEvent::EngineReady | EngineEvent::CommandCompleted => {
            Ok(())
        }
        EngineEvent::NavigationStarted { url, generation } => tab.start_navigation(
            navigation_id(generation),
            Url::new(url.clone()).map_err(|reason| TabError::NavigationMismatch {
                expected: None,
                actual: NavigationId::new(reason),
            })?,
        ),
        EngineEvent::NavigationCommitted { url, generation } => {
            let navigation_id = navigation_id(generation);
            tab.commit_navigation(
                &navigation_id,
                Url::new(url.clone()).map_err(|reason| TabError::NavigationMismatch {
                    expected: Some(navigation_id.clone()),
                    actual: NavigationId::new(reason),
                })?,
            )
        }
        EngineEvent::NavigationFinished { .. } => Ok(()),
        EngineEvent::NavigationFailed { generation, .. }
        | EngineEvent::NavigationCancelled { generation } => {
            tab.fail_navigation(&navigation_id(generation))
        }
        EngineEvent::TitleChanged { title } => {
            tab.set_title(TabTitle::new(title.clone()).map_err(|reason| {
                TabError::NavigationMismatch {
                    expected: None,
                    actual: NavigationId::new(reason),
                }
            })?)
        }
        EngineEvent::EngineCrashed { .. } => tab.crash(),
        EngineEvent::EngineExited => tab.begin_close().and_then(|_| tab.complete_close()),
        EngineEvent::CommandCancelled
        | EngineEvent::CommandTimedOut
        | EngineEvent::QueueSaturated
        | EngineEvent::FrameReady => Ok(()),
    }
}

fn navigation_id(generation: &NavigationGeneration) -> NavigationId {
    NavigationId::new(format!("engine-navigation-{}", generation.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser_domain::ids::{EngineInstanceId, ProfileId, TabId};
    use browser_domain::tab::{TabFocus, TabVisibility};
    use engine_api::events::{EngineEvent, NavigationGeneration};

    fn manager() -> TabManager {
        TabManager::new()
    }

    fn create(manager: &mut TabManager, tab: &str, engine: &str) -> TabBinding {
        manager
            .create_tab(
                TabId::new(tab),
                ProfileId::new("profile-default"),
                EngineInstanceId::new(engine),
            )
            .expect("tab creation")
    }

    fn routed(tab: &str, binding: &TabBinding, event: EngineEvent) -> RoutedEngineEvent {
        RoutedEngineEvent::new(TabId::new(tab), binding.clone(), event)
    }

    #[test]
    fn create_binds_tab_to_engine_and_starts_unselected() {
        let mut manager = manager();
        let binding = create(&mut manager, "tab-1", "engine-1");

        assert_eq!(binding.generation(), 1);
        assert_eq!(
            binding.engine_instance_id(),
            &EngineInstanceId::new("engine-1")
        );
        assert_eq!(manager.active_tab(), None);
        assert_eq!(manager.tab_count(), 1);
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().lifecycle(),
            TabLifecycle::Created
        );
    }

    #[test]
    fn duplicate_tab_ids_are_rejected_without_replacing_existing_binding() {
        let mut manager = manager();
        let original = create(&mut manager, "tab-1", "engine-1");
        let result = manager.create_tab(
            TabId::new("tab-1"),
            ProfileId::new("profile-other"),
            EngineInstanceId::new("engine-2"),
        );

        assert!(matches!(result, Err(TabManagerError::DuplicateTab { .. })));
        assert_eq!(manager.binding(&TabId::new("tab-1")), Some(&original));
        assert_eq!(manager.tab_count(), 1);
    }

    #[test]
    fn selection_cannot_cross_closed_tabs_and_updates_visibility_and_focus() {
        let mut manager = manager();
        create(&mut manager, "tab-1", "engine-1");
        create(&mut manager, "tab-2", "engine-2");

        manager
            .select_tab(&TabId::new("tab-1"))
            .expect("select first");
        manager
            .select_tab(&TabId::new("tab-2"))
            .expect("select second");
        assert_eq!(manager.active_tab(), Some(&TabId::new("tab-2")));
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().focus_state(),
            TabFocus::Unfocused
        );
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().visibility(),
            TabVisibility::Hidden
        );
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().focus_state(),
            TabFocus::Focused
        );
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().visibility(),
            TabVisibility::Visible
        );

        manager
            .close_tab(&TabId::new("tab-2"))
            .expect("close selected");
        assert_eq!(manager.active_tab(), None);
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().lifecycle(),
            TabLifecycle::Closed
        );
        assert!(matches!(
            manager.select_tab(&TabId::new("tab-2")),
            Err(TabManagerError::ClosedTab { .. })
        ));
    }

    #[test]
    fn events_are_fenced_by_tab_and_engine_binding() {
        let mut manager = manager();
        let first = create(&mut manager, "tab-1", "engine-1");
        let second = create(&mut manager, "tab-2", "engine-2");

        let wrong_engine = manager.route_event(&routed(
            "tab-2",
            &first,
            EngineEvent::NavigationStarted {
                url: "https://wrong.example".into(),
                generation: NavigationGeneration(1),
            },
        ));
        assert!(matches!(
            wrong_engine,
            Err(TabManagerError::StaleBinding { .. })
        ));
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().lifecycle(),
            TabLifecycle::Created
        );

        manager
            .route_event(&routed(
                "tab-2",
                &second,
                EngineEvent::NavigationStarted {
                    url: "https://right.example".into(),
                    generation: NavigationGeneration(1),
                },
            ))
            .expect("current binding routes");
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().lifecycle(),
            TabLifecycle::Loading
        );
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().lifecycle(),
            TabLifecycle::Created
        );
    }

    #[test]
    fn stale_binding_after_rebind_cannot_mutate_tab() {
        let mut manager = manager();
        let old = create(&mut manager, "tab-1", "engine-1");
        let current = manager
            .rebind_engine(&TabId::new("tab-1"), EngineInstanceId::new("engine-2"))
            .expect("rebind engine");
        assert_eq!(current.generation(), 2);

        let stale = manager.route_event(&routed(
            "tab-1",
            &old,
            EngineEvent::EngineCrashed {
                reason: "old".into(),
            },
        ));
        assert!(matches!(stale, Err(TabManagerError::StaleBinding { .. })));
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().lifecycle(),
            TabLifecycle::Created
        );

        manager
            .route_event(&routed(
                "tab-1",
                &current,
                EngineEvent::NavigationStarted {
                    url: "https://current.example".into(),
                    generation: NavigationGeneration(3),
                },
            ))
            .expect("current binding routes");
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().lifecycle(),
            TabLifecycle::Loading
        );
    }

    #[test]
    fn close_preserves_closed_record_and_rejects_late_events() {
        let mut manager = manager();
        let binding = create(&mut manager, "tab-1", "engine-1");
        manager.close_tab(&TabId::new("tab-1")).expect("close tab");

        let late = manager.route_event(&routed(
            "tab-1",
            &binding,
            EngineEvent::TitleChanged {
                title: "late".into(),
            },
        ));
        assert!(matches!(late, Err(TabManagerError::ClosedTab { .. })));
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().title().as_str(),
            ""
        );
        assert_eq!(manager.tab_count(), 1);
    }

    #[test]
    fn shutdown_closes_every_live_tab_and_clears_selection() {
        let mut manager = manager();
        create(&mut manager, "tab-1", "engine-1");
        create(&mut manager, "tab-2", "engine-2");
        manager.select_tab(&TabId::new("tab-1")).unwrap();

        let closed = manager.shutdown().expect("shutdown");
        assert_eq!(closed, 2);
        assert_eq!(manager.active_tab(), None);
        assert_eq!(
            manager.tab(&TabId::new("tab-1")).unwrap().lifecycle(),
            TabLifecycle::Closed
        );
        assert_eq!(
            manager.tab(&TabId::new("tab-2")).unwrap().lifecycle(),
            TabLifecycle::Closed
        );
        assert!(matches!(
            manager.shutdown(),
            Err(TabManagerError::AlreadyShutdown)
        ));
    }

    #[test]
    fn navigation_events_update_only_the_bound_tab() {
        let mut manager = manager();
        let binding = create(&mut manager, "tab-1", "engine-1");
        let tab_id = TabId::new("tab-1");
        manager
            .route_event(&routed(
                "tab-1",
                &binding,
                EngineEvent::NavigationStarted {
                    url: "https://example.test".into(),
                    generation: NavigationGeneration(9),
                },
            ))
            .unwrap();
        manager
            .route_event(&routed(
                "tab-1",
                &binding,
                EngineEvent::NavigationCommitted {
                    url: "https://example.test".into(),
                    generation: NavigationGeneration(9),
                },
            ))
            .unwrap();

        let tab = manager.tab(&tab_id).unwrap();
        assert_eq!(tab.lifecycle(), TabLifecycle::Ready);
        assert_eq!(tab.current_url().unwrap().as_str(), "https://example.test");
    }
}
