//! Bookmark repository — add, edit, remove, query.
//!
//! Pure domain logic for user bookmarks. No cloud sync, no import formats.
//! The `BookmarkRepository` trait is implemented by the storage layer.

use crate::ids::ProfileId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum title length for a bookmark.
pub const MAX_BOOKMARK_TITLE_LEN: usize = 1024;
/// Maximum URL length for a bookmark.
pub const MAX_BOOKMARK_URL_LEN: usize = 8192;

/// A single bookmark entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Bookmark identifier.
    pub id: String,
    /// Bookmark URL.
    pub url: String,
    /// Bookmark title.
    pub title: String,
    /// When the bookmark was created (Unix epoch seconds).
    pub created_at: u64,
    /// When the bookmark was last updated (Unix epoch seconds).
    pub updated_at: u64,
}

/// Errors from bookmark operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkError {
    /// Bookmark not found.
    NotFound { id: String },
    /// Bookmark with this ID already exists.
    AlreadyExists { id: String },
    /// URL is empty or too long.
    InvalidUrl { reason: String },
    /// Title is too long.
    InvalidTitle { reason: String },
    /// I/O error (abstracted).
    Io { reason: String },
}

impl fmt::Display for BookmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { id } => write!(f, "bookmark not found: {id}"),
            Self::AlreadyExists { id } => write!(f, "bookmark already exists: {id}"),
            Self::InvalidUrl { reason } => write!(f, "invalid bookmark URL: {reason}"),
            Self::InvalidTitle { reason } => write!(f, "invalid bookmark title: {reason}"),
            Self::Io { reason } => write!(f, "bookmark I/O error: {reason}"),
        }
    }
}

impl std::error::Error for BookmarkError {}

/// Repository abstraction for bookmark persistence.
pub trait BookmarkRepository: fmt::Debug {
    fn add(&mut self, profile_id: &ProfileId, bookmark: Bookmark) -> Result<(), BookmarkError>;
    fn update(&mut self, profile_id: &ProfileId, bookmark: Bookmark) -> Result<(), BookmarkError>;
    fn remove(&mut self, profile_id: &ProfileId, id: &str) -> Result<(), BookmarkError>;
    fn get(&self, profile_id: &ProfileId, id: &str) -> Option<&Bookmark>;
    fn list(&self, profile_id: &ProfileId) -> Vec<Bookmark>;
    fn clear(&mut self, profile_id: &ProfileId) -> Result<(), BookmarkError>;
}

/// The bookmark manager with validation.
pub struct BookmarkManager<R: BookmarkRepository> {
    repository: R,
    next_id: u64,
}

impl<R: BookmarkRepository> BookmarkManager<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            next_id: 1,
        }
    }

    fn next_id(&mut self) -> String {
        let id = format!("bm-{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add(
        &mut self,
        profile_id: &ProfileId,
        url: &str,
        title: &str,
        now: u64,
    ) -> Result<Bookmark, BookmarkError> {
        validate_url(url)?;
        validate_title(title)?;
        let bookmark = Bookmark {
            id: self.next_id(),
            url: url.to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
        };
        self.repository.add(profile_id, bookmark.clone())?;
        Ok(bookmark)
    }

    pub fn update(
        &mut self,
        profile_id: &ProfileId,
        id: &str,
        url: Option<&str>,
        title: Option<&str>,
        now: u64,
    ) -> Result<Bookmark, BookmarkError> {
        let existing = self
            .repository
            .get(profile_id, id)
            .ok_or(BookmarkError::NotFound { id: id.to_string() })?;
        let updated = Bookmark {
            id: existing.id.clone(),
            url: match url {
                Some(u) => {
                    validate_url(u)?;
                    u.to_string()
                }
                None => existing.url.clone(),
            },
            title: match title {
                Some(t) => {
                    validate_title(t)?;
                    t.to_string()
                }
                None => existing.title.clone(),
            },
            created_at: existing.created_at,
            updated_at: now,
        };
        self.repository.update(profile_id, updated.clone())?;
        Ok(updated)
    }

    pub fn remove(&mut self, profile_id: &ProfileId, id: &str) -> Result<(), BookmarkError> {
        self.repository.remove(profile_id, id)
    }

    pub fn get(&self, profile_id: &ProfileId, id: &str) -> Option<&Bookmark> {
        self.repository.get(profile_id, id)
    }

    pub fn list(&self, profile_id: &ProfileId) -> Vec<Bookmark> {
        self.repository.list(profile_id)
    }

    pub fn clear(&mut self, profile_id: &ProfileId) -> Result<(), BookmarkError> {
        self.repository.clear(profile_id)
    }
}

fn validate_url(url: &str) -> Result<(), BookmarkError> {
    if url.is_empty() {
        return Err(BookmarkError::InvalidUrl {
            reason: "empty URL".into(),
        });
    }
    if url.len() > MAX_BOOKMARK_URL_LEN {
        return Err(BookmarkError::InvalidUrl {
            reason: format!("URL exceeds maximum length of {MAX_BOOKMARK_URL_LEN}"),
        });
    }
    Ok(())
}

fn validate_title(title: &str) -> Result<(), BookmarkError> {
    if title.len() > MAX_BOOKMARK_TITLE_LEN {
        return Err(BookmarkError::InvalidTitle {
            reason: format!("title exceeds maximum length of {MAX_BOOKMARK_TITLE_LEN}"),
        });
    }
    Ok(())
}

/// In-memory bookmark repository for testing.
#[derive(Debug, Default)]
pub struct InMemoryBookmarkRepository {
    bookmarks: std::collections::HashMap<String, Vec<Bookmark>>,
}

impl InMemoryBookmarkRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BookmarkRepository for InMemoryBookmarkRepository {
    fn add(&mut self, profile_id: &ProfileId, bookmark: Bookmark) -> Result<(), BookmarkError> {
        let entries = self.bookmarks.entry(profile_id.to_string()).or_default();
        if entries.iter().any(|b| b.id == bookmark.id) {
            return Err(BookmarkError::AlreadyExists {
                id: bookmark.id.clone(),
            });
        }
        entries.push(bookmark);
        Ok(())
    }

    fn update(&mut self, profile_id: &ProfileId, bookmark: Bookmark) -> Result<(), BookmarkError> {
        let entries = self.bookmarks.entry(profile_id.to_string()).or_default();
        if let Some(existing) = entries.iter_mut().find(|b| b.id == bookmark.id) {
            *existing = bookmark;
            Ok(())
        } else {
            Err(BookmarkError::NotFound {
                id: bookmark.id.clone(),
            })
        }
    }

    fn remove(&mut self, profile_id: &ProfileId, id: &str) -> Result<(), BookmarkError> {
        let entries = self.bookmarks.entry(profile_id.to_string()).or_default();
        let before = entries.len();
        entries.retain(|b| b.id != id);
        if entries.len() == before {
            Err(BookmarkError::NotFound { id: id.to_string() })
        } else {
            Ok(())
        }
    }

    fn get(&self, profile_id: &ProfileId, id: &str) -> Option<&Bookmark> {
        self.bookmarks
            .get(&profile_id.to_string())
            .and_then(|entries| entries.iter().find(|b| b.id == id))
    }

    fn list(&self, profile_id: &ProfileId) -> Vec<Bookmark> {
        self.bookmarks
            .get(&profile_id.to_string())
            .cloned()
            .unwrap_or_default()
    }

    fn clear(&mut self, profile_id: &ProfileId) -> Result<(), BookmarkError> {
        self.bookmarks.remove(&profile_id.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(id: &str) -> ProfileId {
        ProfileId::new(id)
    }

    #[test]
    fn add_and_get_bookmark() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let bm = manager
            .add(&profile, "https://example.com", "Example", 1000)
            .unwrap();
        assert_eq!(bm.url, "https://example.com");
        assert_eq!(bm.title, "Example");

        let got = manager.get(&profile, &bm.id).expect("found");
        assert_eq!(got.url, "https://example.com");
    }

    #[test]
    fn list_returns_all() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        manager.add(&profile, "https://a.com", "A", 1000).unwrap();
        manager.add(&profile, "https://b.com", "B", 2000).unwrap();

        let list = manager.list(&profile);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn update_changes_url_and_title() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let bm = manager
            .add(&profile, "https://old.com", "Old", 1000)
            .unwrap();
        let updated = manager
            .update(&profile, &bm.id, Some("https://new.com"), Some("New"), 2000)
            .unwrap();
        assert_eq!(updated.url, "https://new.com");
        assert_eq!(updated.title, "New");
        assert_eq!(updated.created_at, 1000);
        assert_eq!(updated.updated_at, 2000);
    }

    #[test]
    fn update_partial_keeps_other_fields() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let bm = manager
            .add(&profile, "https://keep.com", "Keep Title", 1000)
            .unwrap();
        let updated = manager
            .update(&profile, &bm.id, None, Some("New Title"), 2000)
            .unwrap();
        assert_eq!(updated.url, "https://keep.com");
        assert_eq!(updated.title, "New Title");
    }

    #[test]
    fn remove_deletes_bookmark() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let bm = manager
            .add(&profile, "https://gone.com", "Gone", 1000)
            .unwrap();
        manager.remove(&profile, &bm.id).unwrap();
        assert!(manager.get(&profile, &bm.id).is_none());
    }

    #[test]
    fn remove_nonexistent_returns_error() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let result = manager.remove(&profile, "nonexistent");
        assert!(matches!(result, Err(BookmarkError::NotFound { .. })));
    }

    #[test]
    fn update_nonexistent_returns_error() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        let result = manager.update(&profile, "missing", Some("https://x.com"), None, 1000);
        assert!(matches!(result, Err(BookmarkError::NotFound { .. })));
    }

    #[test]
    fn invalid_url_rejected() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        assert!(matches!(
            manager.add(&profile, "", "Empty", 1000),
            Err(BookmarkError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn profiles_are_isolated() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let p1 = pid("p1");
        let p2 = pid("p2");

        manager.add(&p1, "https://a.com", "A", 1000).unwrap();
        manager.add(&p2, "https://b.com", "B", 2000).unwrap();

        assert_eq!(manager.list(&p1).len(), 1);
        assert_eq!(manager.list(&p2).len(), 1);
        assert_ne!(manager.list(&p1)[0].url, manager.list(&p2)[0].url);
    }

    #[test]
    fn clear_removes_all() {
        let repo = InMemoryBookmarkRepository::new();
        let mut manager = BookmarkManager::new(repo);
        let profile = pid("p1");

        manager.add(&profile, "https://a.com", "A", 1000).unwrap();
        manager.add(&profile, "https://b.com", "B", 2000).unwrap();
        manager.clear(&profile).unwrap();
        assert!(manager.list(&profile).is_empty());
    }
}
