//! Download UI state: progress, cancel, retry, error and bounded history.
//!
//! This module wraps the brokered `DownloadBroker` with the presentation state
//! the UI needs. The destination is decided by the broker policy alone — a
//! caller can never influence the final path, only the suggested name.

use crate::download_broker::{DownloadBroker, DownloadError, DownloadHandle};
use browser_domain::ids::{ProfileId, Url};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum number of terminal download records kept in the UI history.
pub const MAX_HISTORY_ENTRIES: usize = 50;

/// Errors surfaced by the download UI state manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerError {
    Broker(DownloadError),
    WrongProfile,
    QuotaExceeded,
    NotFound,
    NotActive,
    InvalidRequest(String),
}

impl fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Broker(error) => write!(formatter, "download broker rejected: {error}"),
            Self::WrongProfile => formatter.write_str("download profile mismatch"),
            Self::QuotaExceeded => formatter.write_str("download quota exceeded"),
            Self::NotFound => formatter.write_str("download not found"),
            Self::NotActive => formatter.write_str("download is not active"),
            Self::InvalidRequest(reason) => write!(formatter, "invalid download request: {reason}"),
        }
    }
}

impl std::error::Error for ManagerError {}

impl From<DownloadError> for ManagerError {
    fn from(error: DownloadError) -> Self {
        match error {
            DownloadError::QuotaExceeded => Self::QuotaExceeded,
            DownloadError::WrongProfile => Self::WrongProfile,
            DownloadError::NotFound => Self::NotFound,
            DownloadError::NotActive => Self::NotActive,
            other => Self::Broker(other),
        }
    }
}

/// Lifecycle status of one download as presented to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Bytes are being streamed to a temporary file.
    Active { bytes: u64 },
    /// The download reached a safe final path.
    Completed { final_path: PathBuf },
    /// The download was cancelled by the user or the broker.
    Cancelled,
    /// The download failed with a typed reason (quota, I/O, policy).
    Failed { reason: String },
}

/// One terminal (or active) download record for the UI history list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRecord {
    pub id: u64,
    pub source_url: Url,
    pub suggested_name: String,
    pub status: DownloadStatus,
}

/// State returned for an active download.
#[derive(Debug, Clone)]
pub struct DownloadInfo {
    handle: DownloadHandle,
    bytes: u64,
}

impl DownloadInfo {
    pub const fn id(&self) -> u64 {
        self.handle.id()
    }

    pub fn temp_path(&self) -> &Path {
        self.handle.temp_path()
    }

    pub fn final_path(&self) -> &Path {
        self.handle.final_path()
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Debug)]
struct DownloadEntry {
    record: DownloadRecord,
    handle: Option<DownloadHandle>,
}

/// Presentation/state layer over the brokered download policy.
#[derive(Debug)]
pub struct DownloadManager {
    broker: DownloadBroker,
    active: std::collections::HashMap<u64, DownloadEntry>,
    history: std::collections::VecDeque<DownloadRecord>,
    last_status: std::collections::HashMap<u64, DownloadStatus>,
}

impl DownloadManager {
    pub fn new(
        root: impl AsRef<Path>,
        profile_id: ProfileId,
        max_bytes: u64,
    ) -> Result<Self, ManagerError> {
        Ok(Self {
            broker: DownloadBroker::new(root, profile_id, max_bytes)?,
            active: std::collections::HashMap::new(),
            history: std::collections::VecDeque::new(),
            last_status: std::collections::HashMap::new(),
        })
    }

    pub fn start(
        &mut self,
        profile_id: ProfileId,
        source_url: Url,
        suggested_name: impl Into<String>,
        content_length: Option<u64>,
    ) -> Result<u64, ManagerError> {
        let name = suggested_name.into();
        if name.is_empty() || name.len() > 255 {
            return Err(ManagerError::InvalidRequest(
                "suggested name must be 1..=255 bytes".into(),
            ));
        }
        let handle = self
            .broker
            .start(crate::download_broker::DownloadRequest::new(
                profile_id,
                source_url.clone(),
                name.clone(),
                content_length,
            ))?;
        let id = handle.id();
        self.active.insert(
            id,
            DownloadEntry {
                record: DownloadRecord {
                    id,
                    source_url,
                    suggested_name: name,
                    status: DownloadStatus::Active { bytes: 0 },
                },
                handle: Some(handle),
            },
        );
        self.last_status
            .insert(id, DownloadStatus::Active { bytes: 0 });
        Ok(id)
    }

    pub fn write(&mut self, id: u64, bytes: &[u8]) -> Result<(), ManagerError> {
        let current = self.active.get(&id).ok_or(ManagerError::NotFound)?;
        let base = match &current.record.status {
            DownloadStatus::Active { bytes } => *bytes,
            _ => 0,
        };
        let next = base
            .checked_add(bytes.len() as u64)
            .ok_or(ManagerError::QuotaExceeded)?;
        if let Err(error) = self.broker.write(id, bytes) {
            self.fail(id, &error.to_string());
            return Err(error.into());
        }
        if let Some(entry) = self.active.get_mut(&id) {
            entry.record.status = DownloadStatus::Active { bytes: next };
            if let Some(info) = self.last_status.get_mut(&id) {
                *info = DownloadStatus::Active { bytes: next };
            }
        }
        Ok(())
    }

    pub fn finish(
        &mut self,
        id: u64,
    ) -> Result<crate::download_broker::CompletedDownload, ManagerError> {
        let completed = self.broker.finish(id)?;
        if let Some(mut entry) = self.active.remove(&id) {
            entry.record.status = DownloadStatus::Completed {
                final_path: completed.final_path().to_path_buf(),
            };
            entry.handle = None;
            self.push_history(entry.record);
        }
        self.last_status.insert(
            id,
            DownloadStatus::Completed {
                final_path: completed.final_path().to_path_buf(),
            },
        );
        Ok(completed)
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), ManagerError> {
        self.broker.cancel(id)?;
        if let Some(mut entry) = self.active.remove(&id) {
            entry.record.status = DownloadStatus::Cancelled;
            entry.handle = None;
            self.push_history(entry.record);
        }
        self.last_status.insert(id, DownloadStatus::Cancelled);
        Ok(())
    }

    pub fn interrupt(&mut self, id: u64) -> Result<(), ManagerError> {
        self.broker.interrupt(id)?;
        self.cancel(id)
    }

    pub fn status(&self, id: u64) -> DownloadStatus {
        self.last_status
            .get(&id)
            .cloned()
            .unwrap_or(DownloadStatus::Failed {
                reason: "download not found".into(),
            })
    }

    pub fn info(&self, id: u64) -> Option<DownloadInfo> {
        self.active.get(&id).and_then(|entry| {
            entry.handle.clone().map(|handle| {
                let bytes = match &entry.record.status {
                    DownloadStatus::Active { bytes } => *bytes,
                    _ => 0,
                };
                DownloadInfo { handle, bytes }
            })
        })
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn history(&self) -> Vec<DownloadRecord> {
        self.history.iter().cloned().collect()
    }

    /// Remove temporary `.part` files left by an interrupted session.
    ///
    /// Completed files and user files are never touched. Returns the number
    /// of orphaned parts removed.
    pub fn sweep_orphans(&mut self) -> Result<usize, ManagerError> {
        let mut removed = 0;
        let entries = fs::read_dir(self.broker_root()).map_err(|error| {
            ManagerError::Broker(DownloadError::Io {
                operation: "read download root",
                detail: error.to_string(),
            })
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_part = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(".browser-download-") && name.ends_with(".part")
                });
            if is_part {
                fs::remove_file(&path).map_err(|error| {
                    ManagerError::Broker(DownloadError::Io {
                        operation: "remove orphan part",
                        detail: error.to_string(),
                    })
                })?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn broker_root(&self) -> PathBuf {
        self.broker.root().to_path_buf()
    }

    fn fail(&mut self, id: u64, reason: &str) {
        if let Some(mut entry) = self.active.remove(&id) {
            entry.record.status = DownloadStatus::Failed {
                reason: reason.to_string(),
            };
            entry.handle = None;
            self.push_history(entry.record);
        }
        self.last_status.insert(
            id,
            DownloadStatus::Failed {
                reason: reason.to_string(),
            },
        );
    }

    fn push_history(&mut self, record: DownloadRecord) {
        if self.history.len() == MAX_HISTORY_ENTRIES {
            self.history.pop_back();
        }
        self.history.push_front(record);
    }
}
