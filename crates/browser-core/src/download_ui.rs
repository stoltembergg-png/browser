//! Integration seam from typed UI commands to the download UI state manager.

use browser_domain::ids::{ProfileId, Url};
use browser_domain::ui::{
    validate_navigate_url, validate_suggested_name, CommandEnvelope, EventEnvelope, UiCommand,
    UiEvent, UI_CONTRACT_VERSION,
};
use std::fmt;
use std::path::Path;

use crate::download_manager::{DownloadManager, ManagerError};
use crate::ipc_bridge::{IpcBridge, IpcError};

/// Errors produced while connecting a typed download command to the manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadCoordinatorError {
    Ipc(IpcError),
    Manager(ManagerError),
    InvalidPayload(String),
    UnsupportedCommand(&'static str),
}

impl fmt::Display for DownloadCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipc(error) => write!(formatter, "IPC rejected command: {error}"),
            Self::Manager(error) => write!(formatter, "download manager rejected command: {error}"),
            Self::InvalidPayload(reason) => write!(formatter, "invalid download payload: {reason}"),
            Self::UnsupportedCommand(command) => {
                write!(formatter, "unsupported download command: {command}")
            }
        }
    }
}

impl std::error::Error for DownloadCoordinatorError {}

impl From<IpcError> for DownloadCoordinatorError {
    fn from(error: IpcError) -> Self {
        Self::Ipc(error)
    }
}

impl From<ManagerError> for DownloadCoordinatorError {
    fn from(error: ManagerError) -> Self {
        Self::Manager(error)
    }
}

fn event(event: UiEvent) -> EventEnvelope {
    EventEnvelope {
        version: UI_CONTRACT_VERSION,
        tab_id: None,
        event,
    }
}

/// Owns the command/event seam for the download UI state.
///
/// Download commands are app-level (no `tab_id`) because a download is bound
/// to the profile, not to a tab. The destination is decided by the broker
/// policy; the UI only supplies the suggested name and observes events.
pub struct DownloadUiCoordinator {
    bridge: IpcBridge,
    manager: DownloadManager,
    profile_id: ProfileId,
}

impl DownloadUiCoordinator {
    pub fn new(
        root: impl AsRef<Path>,
        profile_id: ProfileId,
        max_bytes: u64,
    ) -> Result<Self, ManagerError> {
        Ok(Self {
            bridge: IpcBridge::new(),
            manager: DownloadManager::new(root, profile_id.clone(), max_bytes)?,
            profile_id,
        })
    }

    pub fn handle_command(
        &mut self,
        raw: &str,
    ) -> Result<Vec<EventEnvelope>, DownloadCoordinatorError> {
        let envelope: CommandEnvelope = self.bridge.validate_command(raw)?;
        match envelope.command {
            UiCommand::DownloadStart {
                url,
                suggested_name,
                content_length,
            } => self.start(url, suggested_name, content_length),
            UiCommand::DownloadCancel { download_id } => self.cancel(download_id),
            UiCommand::DownloadRetry { download_id } => self.retry(download_id),
            _ => Err(DownloadCoordinatorError::UnsupportedCommand(
                "command handled by another coordinator",
            )),
        }
    }

    pub fn active_count(&self) -> usize {
        self.manager.active_count()
    }

    pub fn history(&self) -> Vec<crate::download_manager::DownloadRecord> {
        self.manager.history()
    }

    fn start(
        &mut self,
        url: String,
        suggested_name: String,
        content_length: Option<u64>,
    ) -> Result<Vec<EventEnvelope>, DownloadCoordinatorError> {
        validate_navigate_url(&url).map_err(DownloadCoordinatorError::InvalidPayload)?;
        validate_suggested_name(&suggested_name)
            .map_err(DownloadCoordinatorError::InvalidPayload)?;
        let source_url = Url::new(&url).map_err(DownloadCoordinatorError::InvalidPayload)?;
        let id = self
            .manager
            .start(
                self.manager_profile(),
                source_url,
                suggested_name.clone(),
                content_length,
            )
            .map_err(DownloadCoordinatorError::Manager)?;
        Ok(vec![
            event(UiEvent::DownloadStarted {
                download_id: id,
                suggested_name,
            }),
            event(UiEvent::DownloadProgress {
                download_id: id,
                bytes: 0,
            }),
        ])
    }

    fn cancel(&mut self, download_id: u64) -> Result<Vec<EventEnvelope>, DownloadCoordinatorError> {
        self.manager
            .cancel(download_id)
            .map_err(DownloadCoordinatorError::Manager)?;
        Ok(vec![event(UiEvent::DownloadCancelled { download_id })])
    }

    fn retry(&mut self, download_id: u64) -> Result<Vec<EventEnvelope>, DownloadCoordinatorError> {
        let records = self.manager.history();
        let record = records
            .iter()
            .find(|record| record.id == download_id)
            .ok_or(DownloadCoordinatorError::Manager(ManagerError::NotFound))?;
        let suggested_name = record.suggested_name.clone();
        let source_url = record.source_url.clone();
        let id = self
            .manager
            .start(
                self.manager_profile(),
                source_url,
                suggested_name.clone(),
                None,
            )
            .map_err(DownloadCoordinatorError::Manager)?;
        Ok(vec![
            event(UiEvent::DownloadStarted {
                download_id: id,
                suggested_name,
            }),
            event(UiEvent::DownloadProgress {
                download_id: id,
                bytes: 0,
            }),
        ])
    }

    fn manager_profile(&self) -> ProfileId {
        self.profile_id.clone()
    }
}

impl Default for DownloadUiCoordinator {
    fn default() -> Self {
        let root = std::env::temp_dir().join("browser-pr041-default");
        Self::new(root, ProfileId::new("profile-default"), 1024 * 1024).expect("coordinator")
    }
}
