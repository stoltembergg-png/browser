//! History repository and navigation commit policy.
//!
//! Records visited URLs, titles, and timestamps per profile. Pure domain
//! logic — no I/O, no search ranking, no sync. The `HistoryRepository` trait
//! is implemented by the platform/storage layer.

use crate::ids::ProfileId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum number of history entries per profile.
pub const MAX_HISTORY_ENTRIES: usize = 100_000;

/// A single history entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The visited URL.
    pub url: String,
    /// The page title at visit time, if known.
    pub title: String,
    /// When the visit occurred (Unix epoch seconds).
    pub visited_at: u64,
}

/// History query filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryQuery {
    /// Maximum number of results.
    pub limit: usize,
    /// Filter by URL prefix, if any.
    pub url_prefix: Option<String>,
    /// Only entries after this timestamp (inclusive).
    pub since: Option<u64>,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            url_prefix: None,
            since: None,
        }
    }
}

/// Errors from history operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    /// Too many entries.
    CapacityExceeded { current: usize, max: usize },
    /// URL is empty or invalid.
    InvalidUrl { reason: String },
    /// I/O error (abstracted).
    Io { reason: String },
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded { current, max } => {
                write!(f, "history capacity exceeded: {current} > {max}")
            }
            Self::InvalidUrl { reason } => write!(f, "invalid history URL: {reason}"),
            Self::Io { reason } => write!(f, "history I/O error: {reason}"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// Repository abstraction for persistence.
pub trait HistoryRepository: fmt::Debug {
    /// Record a visit.
    fn record_visit(
        &mut self,
        profile_id: &ProfileId,
        entry: HistoryEntry,
    ) -> Result<(), HistoryError>;

    /// Query history entries.
    fn query(
        &self,
        profile_id: &ProfileId,
        query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, HistoryError>;

    /// Clear all history for a profile.
    fn clear(&mut self, profile_id: &ProfileId) -> Result<(), HistoryError>;

    /// Count entries for a profile.
    fn count(&self, profile_id: &ProfileId) -> usize;
}

/// Navigation commit policy — decides when a navigation should be recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationCommitPolicy {
    /// Record only successful navigations (committed or finished).
    OnCommit,
    /// Record all navigation attempts, including failures.
    OnStart,
}

/// The history manager — records visits and queries entries.
pub struct HistoryManager<R: HistoryRepository> {
    repository: R,
    policy: NavigationCommitPolicy,
}

impl<R: HistoryRepository> HistoryManager<R> {
    pub fn new(repository: R, policy: NavigationCommitPolicy) -> Self {
        Self { repository, policy }
    }

    /// Record a visited URL with title.
    pub fn record_visit(
        &mut self,
        profile_id: &ProfileId,
        url: &str,
        title: &str,
        visited_at: u64,
    ) -> Result<(), HistoryError> {
        if url.is_empty() {
            return Err(HistoryError::InvalidUrl {
                reason: "empty URL".into(),
            });
        }
        let entry = HistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            visited_at,
        };
        self.repository.record_visit(profile_id, entry)
    }

    /// Query history for a profile.
    pub fn query(
        &self,
        profile_id: &ProfileId,
        query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, HistoryError> {
        self.repository.query(profile_id, query)
    }

    /// Clear history for a profile.
    pub fn clear(&mut self, profile_id: &ProfileId) -> Result<(), HistoryError> {
        self.repository.clear(profile_id)
    }

    /// Get the commit policy.
    pub fn policy(&self) -> NavigationCommitPolicy {
        self.policy
    }

    /// Count entries for a profile.
    pub fn count(&self, profile_id: &ProfileId) -> usize {
        self.repository.count(profile_id)
    }

    /// Consume the manager and return the underlying repository.
    pub fn into_repository(self) -> R {
        self.repository
    }
}

/// In-memory history repository for testing.
#[derive(Debug, Default)]
pub struct InMemoryHistoryRepository {
    entries: std::collections::HashMap<String, Vec<HistoryEntry>>,
}

impl InMemoryHistoryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl HistoryRepository for InMemoryHistoryRepository {
    fn record_visit(
        &mut self,
        profile_id: &ProfileId,
        entry: HistoryEntry,
    ) -> Result<(), HistoryError> {
        let entries = self.entries.entry(profile_id.to_string()).or_default();
        if entries.len() >= MAX_HISTORY_ENTRIES {
            return Err(HistoryError::CapacityExceeded {
                current: entries.len(),
                max: MAX_HISTORY_ENTRIES,
            });
        }
        entries.push(entry);
        Ok(())
    }

    fn query(
        &self,
        profile_id: &ProfileId,
        query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, HistoryError> {
        let entries = self
            .entries
            .get(&profile_id.to_string())
            .cloned()
            .unwrap_or_default();
        let mut filtered: Vec<HistoryEntry> = entries
            .into_iter()
            .filter(|e| {
                if let Some(ref prefix) = query.url_prefix {
                    e.url.starts_with(prefix)
                } else {
                    true
                }
            })
            .filter(|e| {
                if let Some(since) = query.since {
                    e.visited_at >= since
                } else {
                    true
                }
            })
            .collect();
        // Sort by most recent first
        filtered.sort_by_key(|b| std::cmp::Reverse(b.visited_at));
        filtered.truncate(query.limit);
        Ok(filtered)
    }

    fn clear(&mut self, profile_id: &ProfileId) -> Result<(), HistoryError> {
        self.entries.remove(&profile_id.to_string());
        Ok(())
    }

    fn count(&self, profile_id: &ProfileId) -> usize {
        self.entries
            .get(&profile_id.to_string())
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(id: &str) -> ProfileId {
        ProfileId::new(id)
    }

    #[test]
    fn record_and_query() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        manager
            .record_visit(&profile, "https://a.com", "A", 1000)
            .unwrap();
        manager
            .record_visit(&profile, "https://b.com", "B", 2000)
            .unwrap();
        manager
            .record_visit(&profile, "https://c.com", "C", 3000)
            .unwrap();

        let results = manager.query(&profile, &HistoryQuery::default()).unwrap();
        assert_eq!(results.len(), 3);
        // Most recent first
        assert_eq!(results[0].url, "https://c.com");
        assert_eq!(results[1].url, "https://b.com");
        assert_eq!(results[2].url, "https://a.com");
    }

    #[test]
    fn query_with_limit() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        for i in 0..10 {
            manager
                .record_visit(&profile, &format!("https://e{i}.com"), "T", 1000 + i)
                .unwrap();
        }

        let query = HistoryQuery {
            limit: 5,
            ..Default::default()
        };
        let results = manager.query(&profile, &query).unwrap();
        assert_eq!(results.len(), 5);
        assert_eq!(results[0].url, "https://e9.com");
    }

    #[test]
    fn query_with_url_prefix() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        manager
            .record_visit(&profile, "https://example.com/a", "A", 1000)
            .unwrap();
        manager
            .record_visit(&profile, "https://other.com/b", "B", 2000)
            .unwrap();
        manager
            .record_visit(&profile, "https://example.com/c", "C", 3000)
            .unwrap();

        let query = HistoryQuery {
            limit: 100,
            url_prefix: Some("https://example.com".into()),
            since: None,
        };
        let results = manager.query(&profile, &query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|e| e.url.starts_with("https://example.com")));
    }

    #[test]
    fn query_with_since_filter() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        manager
            .record_visit(&profile, "https://old.com", "Old", 100)
            .unwrap();
        manager
            .record_visit(&profile, "https://new.com", "New", 2000)
            .unwrap();

        let query = HistoryQuery {
            limit: 100,
            url_prefix: None,
            since: Some(1000),
        };
        let results = manager.query(&profile, &query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://new.com");
    }

    #[test]
    fn clear_removes_all() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        manager
            .record_visit(&profile, "https://a.com", "A", 1000)
            .unwrap();
        assert_eq!(manager.count(&profile), 1);

        manager.clear(&profile).unwrap();
        assert_eq!(manager.count(&profile), 0);
    }

    #[test]
    fn empty_url_rejected() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("p1");

        let result = manager.record_visit(&profile, "", "Title", 1000);
        assert!(matches!(result, Err(HistoryError::InvalidUrl { .. })));
    }

    #[test]
    fn separate_profiles_are_isolated() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);

        let p1 = pid("p1");
        let p2 = pid("p2");

        manager
            .record_visit(&p1, "https://a.com", "A", 1000)
            .unwrap();
        manager
            .record_visit(&p2, "https://b.com", "B", 2000)
            .unwrap();

        assert_eq!(manager.count(&p1), 1);
        assert_eq!(manager.count(&p2), 1);

        let r1 = manager.query(&p1, &HistoryQuery::default()).unwrap();
        let r2 = manager.query(&p2, &HistoryQuery::default()).unwrap();
        assert_eq!(r1[0].url, "https://a.com");
        assert_eq!(r2[0].url, "https://b.com");
    }

    #[test]
    fn private_mode_clears_on_exit() {
        let repo = InMemoryHistoryRepository::new();
        let mut manager = HistoryManager::new(repo, NavigationCommitPolicy::OnCommit);
        let profile = pid("private");

        manager
            .record_visit(&profile, "https://private.com", "P", 1000)
            .unwrap();
        assert_eq!(manager.count(&profile), 1);

        // Private mode exit clears history
        manager.clear(&profile).unwrap();
        assert_eq!(manager.count(&profile), 0);

        let results = manager.query(&profile, &HistoryQuery::default()).unwrap();
        assert!(results.is_empty());
    }
}
