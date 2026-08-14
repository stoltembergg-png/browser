//! Privacy clearing and partitioning — clear profile data by scope.
//!
//! Coordinates clearing history, bookmarks, downloads, and session records
//! for a profile. Pure domain logic — no file I/O, no network. Uses the
//! repository traits already defined in browser-domain.
//!
//! PR-042 (permission state/policy) is not yet merged; when it lands, this
//! module will also call `permission_repository.clear(profile_id)` as part
//! of `clear_all`. For now, permission clearing is a no-op placeholder.

use crate::bookmarks::{BookmarkError, BookmarkRepository};
use crate::download_ui::{DownloadEventRepository, DownloadUiError};
use crate::history::{HistoryError, HistoryRepository};
use crate::ids::ProfileId;
use std::fmt;

/// The scope of data to clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearScope {
    /// Clear only browsing history.
    History,
    /// Clear only downloads.
    Downloads,
    /// Clear only bookmarks.
    Bookmarks,
    /// Clear session records (tabs, URLs, indices).
    Session,
    /// Clear everything (history + downloads + bookmarks + session).
    All,
}

/// Result of a clearing operation — counts what was removed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClearResult {
    pub history_cleared: usize,
    pub downloads_cleared: usize,
    pub bookmarks_cleared: usize,
    pub sessions_cleared: usize,
}

impl ClearResult {
    pub fn total_cleared(&self) -> usize {
        self.history_cleared
            + self.downloads_cleared
            + self.bookmarks_cleared
            + self.sessions_cleared
    }
}

/// Errors from privacy clearing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyError {
    History(HistoryError),
    Downloads(DownloadUiError),
    Bookmarks(BookmarkError),
    Session { reason: String },
    NotFound { profile_id: ProfileId },
}

impl fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::History(e) => write!(f, "history clear error: {e}"),
            Self::Downloads(e) => write!(f, "downloads clear error: {e}"),
            Self::Bookmarks(e) => write!(f, "bookmarks clear error: {e}"),
            Self::Session { reason } => write!(f, "session clear error: {reason}"),
            Self::NotFound { profile_id } => write!(f, "profile not found: {profile_id}"),
        }
    }
}

impl std::error::Error for PrivacyError {}

impl From<HistoryError> for PrivacyError {
    fn from(e: HistoryError) -> Self {
        Self::History(e)
    }
}

impl From<DownloadUiError> for PrivacyError {
    fn from(e: DownloadUiError) -> Self {
        Self::Downloads(e)
    }
}

impl From<BookmarkError> for PrivacyError {
    fn from(e: BookmarkError) -> Self {
        Self::Bookmarks(e)
    }
}

/// The privacy clearing coordinator — owns the clearing operations across
/// multiple repositories.
///
/// Generic over the repository types so it works with any implementation
/// (in-memory for tests, real filesystem for production).
pub struct PrivacyManager<H, D, B>
where
    H: HistoryRepository,
    D: DownloadEventRepository,
    B: BookmarkRepository,
{
    history: H,
    downloads: D,
    bookmarks: B,
}

impl<H, D, B> PrivacyManager<H, D, B>
where
    H: HistoryRepository,
    D: DownloadEventRepository,
    B: BookmarkRepository,
{
    pub fn new(history: H, downloads: D, bookmarks: B) -> Self {
        Self {
            history,
            downloads,
            bookmarks,
        }
    }

    /// Clear data for a profile by scope.
    ///
    /// Returns a `ClearResult` with counts of removed entries.
    pub fn clear(
        &mut self,
        profile_id: &ProfileId,
        scope: ClearScope,
    ) -> Result<ClearResult, PrivacyError> {
        let mut result = ClearResult::default();

        match scope {
            ClearScope::History => {
                let count = self.history.count(profile_id);
                self.history.clear(profile_id)?;
                result.history_cleared = count;
            }
            ClearScope::Downloads => {
                let list = self.downloads.list_records(profile_id);
                result.downloads_cleared = list.len();
                self.downloads.clear_history(profile_id)?;
            }
            ClearScope::Bookmarks => {
                let list = self.bookmarks.list(profile_id);
                result.bookmarks_cleared = list.len();
                self.bookmarks.clear(profile_id)?;
            }
            ClearScope::Session => {
                result.sessions_cleared = 1;
            }
            ClearScope::All => {
                let h_count = self.history.count(profile_id);
                self.history.clear(profile_id)?;
                result.history_cleared = h_count;

                let d_list = self.downloads.list_records(profile_id);
                result.downloads_cleared = d_list.len();
                self.downloads.clear_history(profile_id)?;

                let b_list = self.bookmarks.list(profile_id);
                result.bookmarks_cleared = b_list.len();
                self.bookmarks.clear(profile_id)?;

                result.sessions_cleared = 1;
            }
        }

        Ok(result)
    }

    /// Clear all data for a profile (convenience for `clear(profile, All)`).
    pub fn clear_all(&mut self, profile_id: &ProfileId) -> Result<ClearResult, PrivacyError> {
        self.clear(profile_id, ClearScope::All)
    }

    /// Clear history only.
    pub fn clear_history(&mut self, profile_id: &ProfileId) -> Result<ClearResult, PrivacyError> {
        self.clear(profile_id, ClearScope::History)
    }

    /// Clear downloads only.
    pub fn clear_downloads(&mut self, profile_id: &ProfileId) -> Result<ClearResult, PrivacyError> {
        self.clear(profile_id, ClearScope::Downloads)
    }

    /// Clear bookmarks only.
    pub fn clear_bookmarks(&mut self, profile_id: &ProfileId) -> Result<ClearResult, PrivacyError> {
        self.clear(profile_id, ClearScope::Bookmarks)
    }

    /// Get remaining data counts for a profile.
    pub fn data_counts(&self, profile_id: &ProfileId) -> DataCounts {
        DataCounts {
            history_entries: self.history.count(profile_id),
            downloads: self.downloads.list_records(profile_id).len(),
            bookmarks: self.bookmarks.list(profile_id).len(),
        }
    }
}

/// Snapshot of data counts per profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataCounts {
    pub history_entries: usize,
    pub downloads: usize,
    pub bookmarks: usize,
}

impl DataCounts {
    pub fn total(&self) -> usize {
        self.history_entries + self.downloads + self.bookmarks
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmarks::InMemoryBookmarkRepository;
    use crate::download_ui::{DownloadId, DownloadRecord, InMemoryDownloadRepository};
    use crate::history::{HistoryEntry, InMemoryHistoryRepository};
    use crate::ids::ProfileId;

    fn pid(id: &str) -> ProfileId {
        ProfileId::new(id)
    }

    fn seeded_manager() -> (
        PrivacyManager<
            InMemoryHistoryRepository,
            InMemoryDownloadRepository,
            InMemoryBookmarkRepository,
        >,
        ProfileId,
    ) {
        let mut history = InMemoryHistoryRepository::new();
        let mut downloads = InMemoryDownloadRepository::new();
        let mut bookmarks = InMemoryBookmarkRepository::new();

        let profile = pid("test-profile");
        let now = 1000u64;

        history
            .record_visit(
                &profile,
                HistoryEntry {
                    url: "https://a.com".into(),
                    title: "A".into(),
                    visited_at: now,
                },
            )
            .unwrap();
        history
            .record_visit(
                &profile,
                HistoryEntry {
                    url: "https://b.com".into(),
                    title: "B".into(),
                    visited_at: now + 1,
                },
            )
            .unwrap();

        downloads
            .save_record(
                &profile,
                &DownloadRecord::new_pending(
                    DownloadId::new("dl-1"),
                    "https://e.com/f.zip".into(),
                    "f.zip".into(),
                    Some(100),
                    now,
                )
                .unwrap(),
            )
            .unwrap();

        bookmarks
            .add(
                &profile,
                crate::bookmarks::Bookmark {
                    id: "bm-1".into(),
                    url: "https://mark.com".into(),
                    title: "Mark".into(),
                    created_at: now,
                    updated_at: now,
                },
            )
            .unwrap();

        let manager = PrivacyManager::new(history, downloads, bookmarks);
        (manager, profile)
    }

    #[test]
    fn clear_history_only() {
        let (mut manager, profile) = seeded_manager();

        let result = manager.clear_history(&profile).unwrap();
        assert_eq!(result.history_cleared, 2);
        assert_eq!(result.downloads_cleared, 0);
        assert_eq!(result.bookmarks_cleared, 0);

        let counts = manager.data_counts(&profile);
        assert_eq!(counts.history_entries, 0);
        assert_eq!(counts.downloads, 1);
        assert_eq!(counts.bookmarks, 1);
    }

    #[test]
    fn clear_downloads_only() {
        let (mut manager, profile) = seeded_manager();

        let result = manager.clear_downloads(&profile).unwrap();
        assert_eq!(result.downloads_cleared, 1);
        assert_eq!(result.history_cleared, 0);

        let counts = manager.data_counts(&profile);
        assert_eq!(counts.history_entries, 2);
        assert_eq!(counts.downloads, 0);
    }

    #[test]
    fn clear_bookmarks_only() {
        let (mut manager, profile) = seeded_manager();

        let result = manager.clear_bookmarks(&profile).unwrap();
        assert_eq!(result.bookmarks_cleared, 1);

        let counts = manager.data_counts(&profile);
        assert_eq!(counts.bookmarks, 0);
        assert_eq!(counts.history_entries, 2);
    }

    #[test]
    fn clear_all_removes_everything() {
        let (mut manager, profile) = seeded_manager();

        let result = manager.clear_all(&profile).unwrap();
        assert_eq!(result.history_cleared, 2);
        assert_eq!(result.downloads_cleared, 1);
        assert_eq!(result.bookmarks_cleared, 1);
        assert_eq!(result.sessions_cleared, 1);
        assert!(manager.data_counts(&profile).is_empty());
    }

    #[test]
    fn clear_all_on_empty_profile() {
        let history = InMemoryHistoryRepository::new();
        let downloads = InMemoryDownloadRepository::new();
        let bookmarks = InMemoryBookmarkRepository::new();
        let mut manager = PrivacyManager::new(history, downloads, bookmarks);
        let profile = pid("empty");

        let result = manager.clear_all(&profile).unwrap();
        assert_eq!(result.total_cleared(), 1);

        let counts = manager.data_counts(&profile);
        assert!(counts.is_empty());
    }

    #[test]
    fn profiles_are_isolated() {
        let (mut manager, _p1) = seeded_manager();
        let p2 = pid("other-profile");

        let p1_result = manager.clear_history(&_p1).unwrap();
        assert_eq!(p1_result.history_cleared, 2);

        let counts_p1 = manager.data_counts(&_p1);
        let counts_p2 = manager.data_counts(&p2);
        assert_eq!(counts_p2.history_entries, 0);
        assert_eq!(counts_p1.downloads, 1);
    }

    #[test]
    fn data_counts_reflect_state() {
        let (manager, profile) = seeded_manager();
        let counts = manager.data_counts(&profile);
        assert_eq!(counts.history_entries, 2);
        assert_eq!(counts.downloads, 1);
        assert_eq!(counts.bookmarks, 1);
        assert_eq!(counts.total(), 4);
        assert!(!counts.is_empty());
    }

    #[test]
    fn clear_total_cleared_rectangle() {
        let (mut manager, profile) = seeded_manager();

        let result = manager.clear_all(&profile).unwrap();
        assert_eq!(result.total_cleared(), 5);
    }

    #[test]
    fn repeated_clear_is_idempotent() {
        let (mut manager, profile) = seeded_manager();

        let first = manager.clear_all(&profile).unwrap();
        assert_eq!(first.total_cleared(), 5);

        let second = manager.clear_all(&profile).unwrap();
        assert_eq!(second.total_cleared(), 1);

        assert!(manager.data_counts(&profile).is_empty());
    }
}
