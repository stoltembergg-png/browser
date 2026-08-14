//! Download UI state — progress, cancel, resume, error, and history.
//!
//! Pure domain logic for tracking download presentation state. No file I/O,
//! no real network, no arbitrary file open. The `DownloadEventRepository`
//! trait is implemented by the storage layer.
//!
//! This module does not implement the download broker (PR-040) or file
//! system operations. It models the observable state of a download that
//! the UI renders and the user can interact with.

use crate::ids::ProfileId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum filename length accepted by the UI state.
pub const MAX_DOWNLOAD_FILENAME_LEN: usize = 512;

/// A download identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DownloadId(pub String);

impl DownloadId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DownloadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The observable lifecycle state of a download from the UI perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    /// Download has been requested but not yet started.
    Pending,
    /// Download is in progress — `bytes_received` is advancing.
    InProgress,
    /// Download was paused by the user or system.
    Paused,
    /// Download completed successfully.
    Completed,
    /// Download failed with an error.
    Failed,
    /// Download was cancelled by the user.
    Cancelled,
}

impl DownloadState {
    /// Whether this state is terminal (no further transitions).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Whether the download can be resumed from this state.
    pub fn can_resume(self) -> bool {
        matches!(self, Self::Paused | Self::Failed)
    }

    /// Whether the download can be cancelled from this state.
    pub fn can_cancel(self) -> bool {
        !self.is_terminal()
    }

    /// Whether the download can be paused from this state.
    pub fn can_pause(self) -> bool {
        matches!(self, Self::InProgress)
    }
}

/// A snapshot of one download's observable state for UI rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: DownloadId,
    /// The source URL being downloaded.
    pub url: String,
    /// The sanitized filename for display.
    pub filename: String,
    /// Total expected bytes, if known. `None` means unknown/content-length missing.
    pub total_bytes: Option<u64>,
    /// Bytes received so far.
    pub bytes_received: u64,
    /// Current state.
    pub state: DownloadState,
    /// Error reason, if the download failed.
    pub error_reason: Option<String>,
    /// When the download was created (Unix epoch seconds).
    pub created_at: u64,
    /// When the download state was last updated (Unix epoch seconds).
    pub updated_at: u64,
}

impl DownloadRecord {
    /// Progress as a fraction in [0.0, 1.0]. Returns `None` if total is unknown.
    pub fn progress_fraction(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                0.0
            } else {
                (self.bytes_received as f64 / total as f64).clamp(0.0, 1.0)
            }
        })
    }

    /// Whether the download is currently receiving bytes.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            DownloadState::Pending | DownloadState::InProgress
        )
    }

    /// Create a new pending download record.
    pub fn new_pending(
        id: DownloadId,
        url: String,
        filename: String,
        total_bytes: Option<u64>,
        now: u64,
    ) -> Result<Self, DownloadUiError> {
        if url.is_empty() {
            return Err(DownloadUiError::InvalidUrl);
        }
        if filename.is_empty() {
            return Err(DownloadUiError::InvalidFilename {
                reason: "empty filename".into(),
            });
        }
        if filename.len() > MAX_DOWNLOAD_FILENAME_LEN {
            return Err(DownloadUiError::InvalidFilename {
                reason: format!("filename exceeds {MAX_DOWNLOAD_FILENAME_LEN} bytes"),
            });
        }
        Ok(Self {
            id,
            url,
            filename,
            total_bytes,
            bytes_received: 0,
            state: DownloadState::Pending,
            error_reason: None,
            created_at: now,
            updated_at: now,
        })
    }
}

/// Errors from download UI state operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadUiError {
    InvalidUrl,
    InvalidFilename {
        reason: String,
    },
    InvalidStateTransition {
        from: DownloadState,
        to: DownloadState,
    },
    NotFound {
        id: DownloadId,
    },
    Io {
        reason: String,
    },
}

impl fmt::Display for DownloadUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid download URL: empty"),
            Self::InvalidFilename { reason } => write!(f, "invalid download filename: {reason}"),
            Self::InvalidStateTransition { from, to } => {
                write!(f, "invalid download state transition: {from:?} -> {to:?}")
            }
            Self::NotFound { id } => write!(f, "download not found: {id}"),
            Self::Io { reason } => write!(f, "download I/O error: {reason}"),
        }
    }
}

impl std::error::Error for DownloadUiError {}

/// Repository abstraction for download history persistence.
pub trait DownloadEventRepository: fmt::Debug {
    fn save_record(
        &mut self,
        profile_id: &ProfileId,
        record: &DownloadRecord,
    ) -> Result<(), DownloadUiError>;
    fn get_record(&self, profile_id: &ProfileId, id: &DownloadId) -> Option<DownloadRecord>;
    fn list_records(&self, profile_id: &ProfileId) -> Vec<DownloadRecord>;
    fn clear_history(&mut self, profile_id: &ProfileId) -> Result<(), DownloadUiError>;
    fn remove_record(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
    ) -> Result<(), DownloadUiError>;
}

/// The download UI manager — tracks active downloads and history.
pub struct DownloadUiManager<R: DownloadEventRepository> {
    repository: R,
}

impl<R: DownloadEventRepository> DownloadUiManager<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Create a new download record in Pending state.
    pub fn create_download(
        &mut self,
        profile_id: &ProfileId,
        id: DownloadId,
        url: &str,
        filename: &str,
        total_bytes: Option<u64>,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let record = DownloadRecord::new_pending(
            id,
            url.to_string(),
            filename.to_string(),
            total_bytes,
            now,
        )?;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Update progress for a download.
    pub fn update_progress(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        bytes_received: u64,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if record.state != DownloadState::Pending && record.state != DownloadState::InProgress {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::InProgress,
            });
        }
        record.bytes_received = bytes_received;
        record.state = DownloadState::InProgress;
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Complete a download.
    pub fn complete_download(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if !record.is_active() && record.state != DownloadState::Paused {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::Completed,
            });
        }
        record.state = DownloadState::Completed;
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Fail a download with a reason.
    pub fn fail_download(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        reason: &str,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if record.state.is_terminal() {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::Failed,
            });
        }
        record.state = DownloadState::Failed;
        record.error_reason = Some(reason.to_string());
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Cancel a download.
    pub fn cancel_download(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if !record.state.can_cancel() {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::Cancelled,
            });
        }
        record.state = DownloadState::Cancelled;
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Pause a download.
    pub fn pause_download(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if !record.state.can_pause() {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::Paused,
            });
        }
        record.state = DownloadState::Paused;
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Resume a paused or failed download.
    pub fn resume_download(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
        now: u64,
    ) -> Result<DownloadRecord, DownloadUiError> {
        let mut record = self.get(profile_id, id)?;
        if !record.state.can_resume() {
            return Err(DownloadUiError::InvalidStateTransition {
                from: record.state,
                to: DownloadState::InProgress,
            });
        }
        record.state = DownloadState::InProgress;
        record.error_reason = None;
        record.updated_at = now;
        self.repository.save_record(profile_id, &record)?;
        Ok(record)
    }

    /// Get a download record.
    pub fn get(
        &self,
        profile_id: &ProfileId,
        id: &DownloadId,
    ) -> Result<DownloadRecord, DownloadUiError> {
        self.repository
            .get_record(profile_id, id)
            .ok_or(DownloadUiError::NotFound { id: id.clone() })
    }

    /// List all downloads for a profile.
    pub fn list(&self, profile_id: &ProfileId) -> Vec<DownloadRecord> {
        self.repository.list_records(profile_id)
    }

    /// Clear download history (removes all records).
    pub fn clear_history(&mut self, profile_id: &ProfileId) -> Result<(), DownloadUiError> {
        self.repository.clear_history(profile_id)
    }

    /// Remove a specific download from history.
    pub fn remove(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
    ) -> Result<(), DownloadUiError> {
        self.repository.remove_record(profile_id, id)
    }
}

/// In-memory download repository for testing.
#[derive(Debug, Default)]
pub struct InMemoryDownloadRepository {
    records: std::collections::HashMap<String, Vec<DownloadRecord>>,
}

impl InMemoryDownloadRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DownloadEventRepository for InMemoryDownloadRepository {
    fn save_record(
        &mut self,
        profile_id: &ProfileId,
        record: &DownloadRecord,
    ) -> Result<(), DownloadUiError> {
        let entries = self.records.entry(profile_id.to_string()).or_default();
        if let Some(existing) = entries.iter_mut().find(|r| r.id == record.id) {
            *existing = record.clone();
        } else {
            entries.push(record.clone());
        }
        Ok(())
    }

    fn get_record(&self, profile_id: &ProfileId, id: &DownloadId) -> Option<DownloadRecord> {
        self.records
            .get(&profile_id.to_string())
            .and_then(|entries| entries.iter().find(|r| &r.id == id).cloned())
    }

    fn list_records(&self, profile_id: &ProfileId) -> Vec<DownloadRecord> {
        self.records
            .get(&profile_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn clear_history(&mut self, profile_id: &ProfileId) -> Result<(), DownloadUiError> {
        self.records.remove(&profile_id.to_string());
        Ok(())
    }

    fn remove_record(
        &mut self,
        profile_id: &ProfileId,
        id: &DownloadId,
    ) -> Result<(), DownloadUiError> {
        let entries = self.records.entry(profile_id.to_string()).or_default();
        let before = entries.len();
        entries.retain(|r| &r.id != id);
        if entries.len() == before {
            Err(DownloadUiError::NotFound { id: id.clone() })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(id: &str) -> ProfileId {
        ProfileId::new(id)
    }

    fn did(id: &str) -> DownloadId {
        DownloadId::new(id)
    }

    #[test]
    fn create_download_starts_pending() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        let record = manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://example.com/file.zip",
                "file.zip",
                Some(1000),
                1000,
            )
            .unwrap();

        assert_eq!(record.state, DownloadState::Pending);
        assert_eq!(record.bytes_received, 0);
        assert_eq!(record.total_bytes, Some(1000));
        assert_eq!(record.progress_fraction(), Some(0.0));
    }

    #[test]
    fn progress_updates_fraction() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(1000),
                1000,
            )
            .unwrap();

        let record = manager
            .update_progress(&profile, &did("dl-1"), 500, 1001)
            .unwrap();

        assert_eq!(record.state, DownloadState::InProgress);
        assert_eq!(record.progress_fraction(), Some(0.5));
    }

    #[test]
    fn unknown_total_shows_none_progress() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        let record = manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                None,
                1000,
            )
            .unwrap();
        assert_eq!(record.progress_fraction(), None);
    }

    #[test]
    fn complete_download() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();
        manager
            .update_progress(&profile, &did("dl-1"), 100, 1001)
            .unwrap();

        let record = manager
            .complete_download(&profile, &did("dl-1"), 1002)
            .unwrap();
        assert_eq!(record.state, DownloadState::Completed);
        assert!(record.state.is_terminal());
    }

    #[test]
    fn cancel_download() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();

        let record = manager
            .cancel_download(&profile, &did("dl-1"), 1001)
            .unwrap();
        assert_eq!(record.state, DownloadState::Cancelled);
        assert!(record.state.is_terminal());
    }

    #[test]
    fn pause_and_resume() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();
        manager
            .update_progress(&profile, &did("dl-1"), 50, 1001)
            .unwrap();

        let paused = manager
            .pause_download(&profile, &did("dl-1"), 1002)
            .unwrap();
        assert_eq!(paused.state, DownloadState::Paused);

        let resumed = manager
            .resume_download(&profile, &did("dl-1"), 1003)
            .unwrap();
        assert_eq!(resumed.state, DownloadState::InProgress);
        assert_eq!(resumed.bytes_received, 50);
    }

    #[test]
    fn fail_and_resume_clears_error() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();
        manager
            .update_progress(&profile, &did("dl-1"), 30, 1001)
            .unwrap();

        let failed = manager
            .fail_download(&profile, &did("dl-1"), "network error", 1002)
            .unwrap();
        assert_eq!(failed.state, DownloadState::Failed);
        assert_eq!(failed.error_reason, Some("network error".into()));

        let resumed = manager
            .resume_download(&profile, &did("dl-1"), 1003)
            .unwrap();
        assert_eq!(resumed.state, DownloadState::InProgress);
        assert_eq!(resumed.error_reason, None);
    }

    #[test]
    fn cancel_terminal_rejected() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();
        manager
            .complete_download(&profile, &did("dl-1"), 1001)
            .unwrap();

        let result = manager.cancel_download(&profile, &did("dl-1"), 1002);
        assert!(matches!(
            result,
            Err(DownloadUiError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn pause_non_inprogress_rejected() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://e.com/f.zip",
                "f.zip",
                Some(100),
                1000,
            )
            .unwrap();
        // Pending — cannot pause
        let result = manager.pause_download(&profile, &did("dl-1"), 1001);
        assert!(matches!(
            result,
            Err(DownloadUiError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn empty_url_rejected() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        let result = manager.create_download(&profile, did("dl-1"), "", "f.zip", None, 1000);
        assert!(matches!(result, Err(DownloadUiError::InvalidUrl)));
    }

    #[test]
    fn empty_filename_rejected() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        let result =
            manager.create_download(&profile, did("dl-1"), "https://e.com", "", None, 1000);
        assert!(matches!(
            result,
            Err(DownloadUiError::InvalidFilename { .. })
        ));
    }

    #[test]
    fn list_and_clear_history() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://a.com/a.zip",
                "a.zip",
                None,
                1000,
            )
            .unwrap();
        manager
            .create_download(
                &profile,
                did("dl-2"),
                "https://b.com/b.zip",
                "b.zip",
                None,
                2000,
            )
            .unwrap();

        assert_eq!(manager.list(&profile).len(), 2);
        manager.clear_history(&profile).unwrap();
        assert!(manager.list(&profile).is_empty());
    }

    #[test]
    fn profiles_are_isolated() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let p1 = pid("p1");
        let p2 = pid("p2");

        manager
            .create_download(&p1, did("dl-1"), "https://a.com/a.zip", "a.zip", None, 1000)
            .unwrap();
        manager
            .create_download(&p2, did("dl-2"), "https://b.com/b.zip", "b.zip", None, 2000)
            .unwrap();

        assert_eq!(manager.list(&p1).len(), 1);
        assert_eq!(manager.list(&p2).len(), 1);
        assert_ne!(manager.list(&p1)[0].url, manager.list(&p2)[0].url);
    }

    #[test]
    fn remove_from_history() {
        let repo = InMemoryDownloadRepository::new();
        let mut manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        manager
            .create_download(
                &profile,
                did("dl-1"),
                "https://a.com/a.zip",
                "a.zip",
                None,
                1000,
            )
            .unwrap();
        manager
            .create_download(
                &profile,
                did("dl-2"),
                "https://b.com/b.zip",
                "b.zip",
                None,
                2000,
            )
            .unwrap();

        manager.remove(&profile, &did("dl-1")).unwrap();
        assert_eq!(manager.list(&profile).len(), 1);
        assert_eq!(manager.list(&profile)[0].id, did("dl-2"));
    }

    #[test]
    fn not_found_error() {
        let repo = InMemoryDownloadRepository::new();
        let manager = DownloadUiManager::new(repo);
        let profile = pid("p1");

        let result = manager.get(&profile, &did("nonexistent"));
        assert!(matches!(result, Err(DownloadUiError::NotFound { .. })));
    }
}
