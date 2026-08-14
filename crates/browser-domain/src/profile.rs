//! Profile storage, locking, and migration runner.
//!
//! Pure domain logic for profile root management, lock acquisition/release,
//! repository abstraction, and schema migration. No real filesystem I/O —
//! the `ProfileStorage` trait is implemented by the platform layer.
//!
//! See ADR-006 for the storage decision.

use crate::ids::ProfileId;
use crate::session::{migrate_session, SessionError, SessionRecord, SESSION_SCHEMA_VERSION};
use std::collections::HashMap;
use std::fmt;

/// Maximum number of concurrent lock attempts before giving up.
pub const MAX_LOCK_ATTEMPTS: u32 = 5;

/// Lock conflict — another process holds the profile lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockError {
    pub profile_id: ProfileId,
    pub reason: String,
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "profile lock error for {}: {}",
            self.profile_id, self.reason
        )
    }
}

impl std::error::Error for LockError {}

/// State of a profile lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    /// Lock is available.
    Unlocked,
    /// Lock is held by this session.
    Locked,
    /// Lock is held by another process (stale detection pending).
    HeldByOther,
}

/// A profile lock handle. Dropping it releases the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileLock {
    profile_id: ProfileId,
    generation: u64,
}

impl ProfileLock {
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Metadata about a profile on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub profile_id: ProfileId,
    pub schema_version: u32,
    pub created_at: u64,
    pub last_used: u64,
}

/// Repository abstraction for reading and writing session records.
/// The platform layer implements this against real filesystem paths.
pub trait SessionRepository: fmt::Debug {
    /// Read the current session record for a profile.
    fn read_session(&self, profile_id: &ProfileId) -> Result<Option<SessionRecord>, SessionError>;

    /// Write a session record for a profile.
    fn write_session(
        &mut self,
        profile_id: &ProfileId,
        record: &SessionRecord,
    ) -> Result<(), SessionError>;

    /// Check if a profile directory exists.
    fn profile_exists(&self, profile_id: &ProfileId) -> bool;

    /// Create a profile directory.
    fn create_profile(&mut self, profile_id: &ProfileId) -> Result<(), ProfileError>;

    /// Get profile metadata.
    fn profile_metadata(&self, profile_id: &ProfileId) -> Option<ProfileMetadata>;
}

/// Errors from profile operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Profile does not exist.
    NotFound { profile_id: ProfileId },
    /// Profile already exists.
    AlreadyExists { profile_id: ProfileId },
    /// Lock is held by another process.
    LockHeld { profile_id: ProfileId },
    /// Lock is stale (process died without releasing).
    LockStale { profile_id: ProfileId },
    /// Migration failed.
    MigrationFailed { reason: String },
    /// Corruption detected.
    Corruption { reason: String },
    /// I/O error (abstracted — no raw OS error).
    Io { reason: String },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { profile_id } => write!(f, "profile not found: {profile_id}"),
            Self::AlreadyExists { profile_id } => {
                write!(f, "profile already exists: {profile_id}")
            }
            Self::LockHeld { profile_id } => {
                write!(f, "profile lock held by another process: {profile_id}")
            }
            Self::LockStale { profile_id } => write!(f, "profile lock is stale: {profile_id}"),
            Self::MigrationFailed { reason } => write!(f, "migration failed: {reason}"),
            Self::Corruption { reason } => write!(f, "profile corruption: {reason}"),
            Self::Io { reason } => write!(f, "profile I/O error: {reason}"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// The profile manager — owns locks, repositories, and migration.
pub struct ProfileManager<R: SessionRepository> {
    repository: R,
    locks: HashMap<ProfileId, ProfileLock>,
    next_lock_generation: u64,
}

impl<R: SessionRepository> ProfileManager<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            locks: HashMap::new(),
            next_lock_generation: 1,
        }
    }

    /// Acquire a lock for a profile. Returns the lock handle.
    pub fn acquire_lock(&mut self, profile_id: &ProfileId) -> Result<ProfileLock, ProfileError> {
        if self.locks.contains_key(profile_id) {
            return Err(ProfileError::LockHeld {
                profile_id: profile_id.clone(),
            });
        }
        let generation = self.next_lock_generation;
        self.next_lock_generation += 1;
        let lock = ProfileLock {
            profile_id: profile_id.clone(),
            generation,
        };
        self.locks.insert(profile_id.clone(), lock.clone());
        Ok(lock)
    }

    /// Release a lock for a profile.
    pub fn release_lock(&mut self, lock: &ProfileLock) {
        self.locks.remove(&lock.profile_id);
    }

    /// Check if a profile is locked.
    pub fn is_locked(&self, profile_id: &ProfileId) -> bool {
        self.locks.contains_key(profile_id)
    }

    /// Load a session for a profile, with migration if needed.
    pub fn load_session(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Option<SessionRecord>, ProfileError> {
        match self.repository.read_session(profile_id) {
            Ok(Some(record)) => {
                if record.version == SESSION_SCHEMA_VERSION {
                    Ok(Some(record))
                } else {
                    migrate_session(record)
                        .map(Some)
                        .map_err(|e| ProfileError::MigrationFailed {
                            reason: e.to_string(),
                        })
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ProfileError::Io {
                reason: e.to_string(),
            }),
        }
    }

    /// Save a session for a profile.
    pub fn save_session(
        &mut self,
        profile_id: &ProfileId,
        record: &SessionRecord,
    ) -> Result<(), ProfileError> {
        if !self.is_locked(profile_id) {
            return Err(ProfileError::LockHeld {
                profile_id: profile_id.clone(),
            });
        }
        self.repository
            .write_session(profile_id, record)
            .map_err(|e| ProfileError::Io {
                reason: e.to_string(),
            })
    }

    /// Create a new profile.
    pub fn create_profile(&mut self, profile_id: &ProfileId) -> Result<(), ProfileError> {
        if self.repository.profile_exists(profile_id) {
            return Err(ProfileError::AlreadyExists {
                profile_id: profile_id.clone(),
            });
        }
        self.repository.create_profile(profile_id)
    }

    /// Get profile metadata.
    pub fn profile_metadata(&self, profile_id: &ProfileId) -> Option<ProfileMetadata> {
        self.repository.profile_metadata(profile_id)
    }

    /// Get a reference to the repository.
    pub fn repository(&self) -> &R {
        &self.repository
    }
}

/// In-memory session repository for testing.
#[derive(Debug, Default)]
pub struct InMemorySessionRepository {
    sessions: HashMap<String, SessionRecord>,
    profiles: HashMap<String, ProfileMetadata>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_profile(&mut self, profile_id: &ProfileId, created_at: u64) {
        self.profiles.insert(
            profile_id.to_string(),
            ProfileMetadata {
                profile_id: profile_id.clone(),
                schema_version: SESSION_SCHEMA_VERSION,
                created_at,
                last_used: created_at,
            },
        );
    }
}

impl SessionRepository for InMemorySessionRepository {
    fn read_session(&self, profile_id: &ProfileId) -> Result<Option<SessionRecord>, SessionError> {
        Ok(self.sessions.get(&profile_id.to_string()).cloned())
    }

    fn write_session(
        &mut self,
        profile_id: &ProfileId,
        record: &SessionRecord,
    ) -> Result<(), SessionError> {
        self.sessions.insert(profile_id.to_string(), record.clone());
        Ok(())
    }

    fn profile_exists(&self, profile_id: &ProfileId) -> bool {
        self.profiles.contains_key(&profile_id.to_string())
    }

    fn create_profile(&mut self, profile_id: &ProfileId) -> Result<(), ProfileError> {
        let now = 1000;
        self.profiles.insert(
            profile_id.to_string(),
            ProfileMetadata {
                profile_id: profile_id.clone(),
                schema_version: SESSION_SCHEMA_VERSION,
                created_at: now,
                last_used: now,
            },
        );
        Ok(())
    }

    fn profile_metadata(&self, profile_id: &ProfileId) -> Option<ProfileMetadata> {
        self.profiles.get(&profile_id.to_string()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(id: &str) -> ProfileId {
        ProfileId::new(id)
    }

    #[test]
    fn acquire_and_release_lock() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("profile-1");

        let lock = manager.acquire_lock(&profile).expect("acquire");
        assert!(manager.is_locked(&profile));

        manager.release_lock(&lock);
        assert!(!manager.is_locked(&profile));
    }

    #[test]
    fn double_lock_rejected() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("profile-1");

        manager.acquire_lock(&profile).expect("first lock");
        let result = manager.acquire_lock(&profile);
        assert!(matches!(result, Err(ProfileError::LockHeld { .. })));
    }

    #[test]
    fn save_requires_lock() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("profile-1");
        let session = SessionRecord::new(profile.clone(), 1000);

        let result = manager.save_session(&profile, &session);
        assert!(matches!(result, Err(ProfileError::LockHeld { .. })));
    }

    #[test]
    fn save_and_load_session_roundtrip() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("profile-1");

        manager.acquire_lock(&profile).expect("lock");
        let session = SessionRecord::new(profile.clone(), 1000);
        manager.save_session(&profile, &session).expect("save");

        let loaded = manager.load_session(&profile).expect("load");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), session);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let repo = InMemorySessionRepository::new();
        let manager = ProfileManager::new(repo);
        let profile = pid("nonexistent");

        let result = manager.load_session(&profile).expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn create_profile_succeeds() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("new-profile");

        manager.create_profile(&profile).expect("create");
        assert!(manager.profile_metadata(&profile).is_some());
    }

    #[test]
    fn create_duplicate_profile_rejected() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);
        let profile = pid("dup");

        manager.create_profile(&profile).expect("first");
        let result = manager.create_profile(&profile);
        assert!(matches!(result, Err(ProfileError::AlreadyExists { .. })));
    }

    #[test]
    fn lock_generations_are_monotonic() {
        let repo = InMemorySessionRepository::new();
        let mut manager = ProfileManager::new(repo);

        let l1 = manager.acquire_lock(&pid("a")).expect("lock a");
        manager.release_lock(&l1);
        let l2 = manager.acquire_lock(&pid("b")).expect("lock b");
        manager.release_lock(&l2);
        let l3 = manager.acquire_lock(&pid("a")).expect("lock a again");

        assert!(l2.generation() > l1.generation());
        assert!(l3.generation() > l2.generation());
    }

    #[test]
    fn migration_on_load() {
        let mut repo = InMemorySessionRepository::new();
        let profile = pid("migrate");
        repo.seed_profile(&profile, 1000);

        // Store a v1 session (current version — no migration needed yet)
        let session = SessionRecord::new(profile.clone(), 1000);
        repo.write_session(&profile, &session).expect("write");

        let manager = ProfileManager::new(repo);
        let loaded = manager.load_session(&profile).expect("load");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().version, SESSION_SCHEMA_VERSION);
    }
}
