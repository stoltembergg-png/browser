//! Transactional session restore and shutdown orchestration.
//!
//! The coordinator enforces quiesce-before-save ordering and uses an append-only
//! journal so an interrupted final write cannot replace the last complete
//! session. Restored tabs are returned as placeholders: this module never
//! dispatches navigation or replays page work.

use browser_domain::ids::{EngineInstanceId, ProfileId, TabId, Url};
use browser_domain::session::{migrate_session, SessionError, SessionRecord};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const JOURNAL_MAGIC: &str = "PR036";
const JOURNAL_VERSION: &str = "1";

/// Lifecycle phase for one shutdown transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Running,
    Quiescing,
    Persisting,
    Committed,
    Aborted,
}

/// Work that must be cancelled before persistence starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingWork {
    commands: usize,
    downloads: usize,
}

impl PendingWork {
    pub const fn new(commands: usize, downloads: usize) -> Self {
        Self {
            commands,
            downloads,
        }
    }

    pub const fn none() -> Self {
        Self::new(0, 0)
    }

    pub const fn commands(self) -> usize {
        self.commands
    }

    pub const fn downloads(self) -> usize {
        self.downloads
    }
}

/// Acknowledgement that new work is blocked and existing work is cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuiesceReceipt {
    cancelled_commands: usize,
    cancelled_downloads: usize,
}

impl QuiesceReceipt {
    pub const fn cancelled_commands(self) -> usize {
        self.cancelled_commands
    }

    pub const fn cancelled_downloads(self) -> usize {
        self.cancelled_downloads
    }
}

/// Durable result of a successfully committed shutdown transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReceipt {
    sequence: u64,
    cancelled_commands: usize,
    cancelled_downloads: usize,
}

impl ShutdownReceipt {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn cancelled_commands(self) -> usize {
        self.cancelled_commands
    }

    pub const fn cancelled_downloads(self) -> usize {
        self.cancelled_downloads
    }
}

/// Errors returned by session restore and shutdown operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionLifecycleError {
    InvalidPhase {
        operation: &'static str,
        phase: SessionPhase,
    },
    JournalConflict,
    Session(SessionError),
    Persistence {
        operation: &'static str,
        detail: String,
    },
}

impl fmt::Display for SessionLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhase { operation, phase } => {
                write!(formatter, "{operation} rejected in session phase {phase:?}")
            }
            Self::JournalConflict => write!(formatter, "session journal conflict"),
            Self::Session(error) => write!(formatter, "invalid session record: {error}"),
            Self::Persistence { operation, detail } => {
                write!(formatter, "{operation} failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SessionLifecycleError {}

impl From<SessionError> for SessionLifecycleError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

/// A session record staged for commit but not yet visible to recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSession {
    sequence: u64,
    record: SessionRecord,
}

/// The last complete record recovered from the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredSession {
    sequence: u64,
    record: SessionRecord,
}

impl RecoveredSession {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn record(&self) -> &SessionRecord {
        &self.record
    }
}

/// A journal-backed session persistence seam.
///
/// A record becomes visible only after the complete line has been written and
/// flushed. A torn final line is ignored on reopen, preserving the newest
/// complete record. This is durable local persistence, not cloud sync.
#[derive(Debug)]
struct DurableSessionJournal {
    path: PathBuf,
    next_sequence: u64,
    pending: Option<PendingSession>,
    committed: Option<RecoveredSession>,
}

impl DurableSessionJournal {
    fn open(path: impl AsRef<Path>) -> Result<Self, SessionLifecycleError> {
        let path = path.as_ref().to_path_buf();
        let mut journal = Self {
            path,
            next_sequence: 0,
            pending: None,
            committed: None,
        };

        if !journal.path.exists() {
            return Ok(journal);
        }

        let mut contents = String::new();
        File::open(&journal.path)
            .and_then(|mut file| file.read_to_string(&mut contents))
            .map_err(|error| SessionLifecycleError::Persistence {
                operation: "read session journal",
                detail: error.to_string(),
            })?;

        let complete_file = contents.ends_with('\n');
        let lines: Vec<&str> = contents.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let torn_final_record = index + 1 == lines.len() && !complete_file;
            match decode_record(line) {
                Ok(Some(recovered)) => {
                    journal.next_sequence = journal.next_sequence.max(recovered.sequence);
                    let is_newer = journal
                        .committed
                        .as_ref()
                        .is_none_or(|current| recovered.sequence > current.sequence);
                    if is_newer {
                        journal.committed = Some(recovered);
                    }
                }
                Ok(None) if torn_final_record => {}
                Ok(None) => {
                    return Err(SessionLifecycleError::Persistence {
                        operation: "parse session journal",
                        detail: "unsupported session record".to_string(),
                    });
                }
                Err(error) if torn_final_record => {
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(journal)
    }

    fn prepare(&mut self, record: SessionRecord) -> Result<PendingSession, SessionLifecycleError> {
        record.validate()?;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let pending = PendingSession {
            sequence: self.next_sequence,
            record,
        };
        self.pending = Some(pending.clone());
        Ok(pending)
    }

    fn commit(&mut self, pending: PendingSession) -> Result<(), SessionLifecycleError> {
        if self.pending.as_ref().map(|value| value.sequence) != Some(pending.sequence) {
            return Err(SessionLifecycleError::JournalConflict);
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                SessionLifecycleError::Persistence {
                    operation: "create session journal directory",
                    detail: error.to_string(),
                }
            })?;
        }
        let encoded = encode_record(pending.sequence, &pending.record)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| SessionLifecycleError::Persistence {
                operation: "open session journal",
                detail: error.to_string(),
            })?;
        file.write_all(encoded.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| SessionLifecycleError::Persistence {
                operation: "flush session journal",
                detail: error.to_string(),
            })?;

        self.committed = Some(RecoveredSession {
            sequence: pending.sequence,
            record: pending.record,
        });
        self.pending = None;
        Ok(())
    }

    fn abort(&mut self, pending: PendingSession) -> Result<(), SessionLifecycleError> {
        if self.pending.as_ref().map(|value| value.sequence) != Some(pending.sequence) {
            return Err(SessionLifecycleError::JournalConflict);
        }
        self.pending = None;
        Ok(())
    }

    fn discard_pending(&mut self, sequence: u64) {
        if self
            .pending
            .as_ref()
            .is_some_and(|value| value.sequence == sequence)
        {
            self.pending = None;
        }
    }

    fn recover(&self) -> Option<RecoveredSession> {
        self.committed.clone()
    }
}

fn encode_record(sequence: u64, record: &SessionRecord) -> Result<String, SessionLifecycleError> {
    let json = record.to_json()?;
    Ok(format!(
        "{JOURNAL_MAGIC}|{JOURNAL_VERSION}|{sequence}|{json}\n"
    ))
}

fn decode_record(line: &str) -> Result<Option<RecoveredSession>, SessionLifecycleError> {
    let mut fields = line.splitn(4, '|');
    if fields.next() != Some(JOURNAL_MAGIC) {
        return Ok(None);
    }
    if fields.next() != Some(JOURNAL_VERSION) {
        return Err(SessionLifecycleError::Persistence {
            operation: "parse session journal",
            detail: "unsupported session journal version".to_string(),
        });
    }
    let sequence = fields
        .next()
        .ok_or_else(|| SessionLifecycleError::Persistence {
            operation: "parse session journal",
            detail: "missing session sequence".to_string(),
        })?
        .parse::<u64>()
        .map_err(|error| SessionLifecycleError::Persistence {
            operation: "parse session journal",
            detail: error.to_string(),
        })?;
    let raw_record = fields
        .next()
        .ok_or_else(|| SessionLifecycleError::Persistence {
            operation: "parse session journal",
            detail: "missing session record".to_string(),
        })?;
    let record = migrate_session(SessionRecord::from_json(raw_record)?)?;
    Ok(Some(RecoveredSession { sequence, record }))
}

/// A restored tab that is safe to display before any navigation is explicitly
/// requested by a higher-level policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredTab {
    tab_id: TabId,
    engine_instance_id: EngineInstanceId,
    current_url: Option<Url>,
    title: String,
    visible: bool,
    created_at: u64,
    disposition: RestoreDisposition,
}

impl RestoredTab {
    pub fn tab_id(&self) -> &TabId {
        &self.tab_id
    }

    pub fn engine_instance_id(&self) -> &EngineInstanceId {
        &self.engine_instance_id
    }

    pub fn current_url(&self) -> Option<&Url> {
        self.current_url.as_ref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn disposition(&self) -> RestoreDisposition {
        self.disposition
    }
}

/// Restore is intentionally a placeholder operation; it does not replay web
/// navigation, forms, downloads, or pending engine commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreDisposition {
    Placeholder,
}

/// A validated session ready for the browser core to materialize explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredSession {
    profile_id: ProfileId,
    tabs: Vec<RestoredTab>,
    active_tab_index: Option<u32>,
    source_sequence: u64,
}

impl RestoredSession {
    fn from_recovered(recovered: RecoveredSession) -> Self {
        let record = recovered.record;
        let tabs = record
            .tabs
            .into_iter()
            .map(|tab| RestoredTab {
                tab_id: tab.tab_id,
                engine_instance_id: tab.engine_instance_id,
                current_url: tab.current_url,
                title: tab.title,
                visible: tab.visible,
                created_at: tab.created_at,
                disposition: RestoreDisposition::Placeholder,
            })
            .collect();
        Self {
            profile_id: record.profile_id,
            tabs,
            active_tab_index: record.active_tab_index,
            source_sequence: recovered.sequence,
        }
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn tabs(&self) -> &[RestoredTab] {
        &self.tabs
    }

    pub const fn active_tab_index(&self) -> Option<u32> {
        self.active_tab_index
    }

    pub const fn source_sequence(&self) -> u64 {
        self.source_sequence
    }
}

/// Coordinates quiesce, transactional session save, and safe restore.
#[derive(Debug)]
pub struct SessionLifecycle {
    journal: DurableSessionJournal,
    phase: SessionPhase,
    cancelled_commands: usize,
    cancelled_downloads: usize,
}

impl SessionLifecycle {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionLifecycleError> {
        Ok(Self {
            journal: DurableSessionJournal::open(path)?,
            phase: SessionPhase::Running,
            cancelled_commands: 0,
            cancelled_downloads: 0,
        })
    }

    pub const fn phase(&self) -> SessionPhase {
        self.phase
    }

    /// Reject work after this point and acknowledge cancellation of pending work.
    pub fn begin_shutdown(
        &mut self,
        pending_work: PendingWork,
    ) -> Result<QuiesceReceipt, SessionLifecycleError> {
        if !matches!(self.phase, SessionPhase::Running | SessionPhase::Aborted) {
            return Err(SessionLifecycleError::InvalidPhase {
                operation: "begin_shutdown",
                phase: self.phase,
            });
        }
        self.phase = SessionPhase::Quiescing;
        self.cancelled_commands = pending_work.commands();
        self.cancelled_downloads = pending_work.downloads();
        Ok(QuiesceReceipt {
            cancelled_commands: self.cancelled_commands,
            cancelled_downloads: self.cancelled_downloads,
        })
    }

    pub fn admit_work(&self) -> Result<(), SessionLifecycleError> {
        if self.phase == SessionPhase::Running {
            Ok(())
        } else {
            Err(SessionLifecycleError::InvalidPhase {
                operation: "admit_work",
                phase: self.phase,
            })
        }
    }

    /// Validate and stage a session after quiesce, without making it recoverable yet.
    pub fn prepare_session(
        &mut self,
        record: SessionRecord,
    ) -> Result<PendingSession, SessionLifecycleError> {
        if self.phase != SessionPhase::Quiescing {
            return Err(SessionLifecycleError::InvalidPhase {
                operation: "prepare_session",
                phase: self.phase,
            });
        }
        let pending = self.journal.prepare(record)?;
        self.phase = SessionPhase::Persisting;
        Ok(pending)
    }

    /// Make the staged record visible only after a complete durable write.
    pub fn commit_session(
        &mut self,
        pending: PendingSession,
    ) -> Result<ShutdownReceipt, SessionLifecycleError> {
        if self.phase != SessionPhase::Persisting {
            return Err(SessionLifecycleError::InvalidPhase {
                operation: "commit_session",
                phase: self.phase,
            });
        }
        let sequence = pending.sequence;
        match self.journal.commit(pending) {
            Ok(()) => {
                self.phase = SessionPhase::Committed;
                Ok(ShutdownReceipt {
                    sequence,
                    cancelled_commands: self.cancelled_commands,
                    cancelled_downloads: self.cancelled_downloads,
                })
            }
            Err(error) => {
                self.journal.discard_pending(sequence);
                self.phase = SessionPhase::Aborted;
                Err(error)
            }
        }
    }

    /// Abort a staged write while preserving the previously committed record.
    pub fn abort_session(&mut self, pending: PendingSession) -> Result<(), SessionLifecycleError> {
        if self.phase != SessionPhase::Persisting {
            return Err(SessionLifecycleError::InvalidPhase {
                operation: "abort_session",
                phase: self.phase,
            });
        }
        self.journal.abort(pending)?;
        self.phase = SessionPhase::Aborted;
        Ok(())
    }

    /// Read the newest complete record as inert placeholders.
    pub fn restore(&self) -> Result<Option<RestoredSession>, SessionLifecycleError> {
        Ok(self.journal.recover().map(RestoredSession::from_recovered))
    }
}
