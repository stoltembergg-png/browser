//! Crash, hang, and abrupt-shutdown recovery policy.
//!
//! The policy is deliberately engine-neutral. It owns recovery state, redacted
//! diagnostics, checkpoint fencing, retry limits, and the rule that a restart
//! never automatically resubmits an in-flight form.

/// Monotonic identity for one engine incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineEpoch(u64);

impl EngineEpoch {
    pub const fn initial() -> Self {
        Self(1)
    }

    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Generation fence carried by engine events after a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFence {
    epoch: EngineEpoch,
    generation: u32,
}

impl EventFence {
    pub const fn new(epoch: EngineEpoch, generation: u32) -> Self {
        Self { epoch, generation }
    }
}

/// Durable state that may be used to restore a tab without replaying a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCheckpoint {
    epoch: EngineEpoch,
    generation: u32,
    url: Option<String>,
    form_in_flight: bool,
}

impl RecoveryCheckpoint {
    pub fn new(
        epoch: EngineEpoch,
        generation: u32,
        url: Option<String>,
        form_in_flight: bool,
    ) -> Self {
        Self {
            epoch,
            generation,
            url,
            form_in_flight,
        }
    }

    pub fn epoch(&self) -> EngineEpoch {
        self.epoch
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn form_in_flight(&self) -> bool {
        self.form_in_flight
    }
}

/// Recovery state visible to the browser core/UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    Running,
    Crashed,
    Hung,
    Restarting,
    Terminal,
}

/// Engine failure categories safe to expose in diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCause {
    Panic,
    OutOfMemory,
    WatchdogTimeout,
}

/// Diagnostics with sensitive detail intentionally removed.
#[derive(Clone, PartialEq, Eq)]
pub struct CrashDiagnostics {
    cause: FailureCause,
    redacted_detail: &'static str,
}

impl std::fmt::Debug for CrashDiagnostics {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CrashDiagnostics")
            .field("cause", &self.cause)
            .field("detail", &self.redacted_detail)
            .finish()
    }
}

impl CrashDiagnostics {
    pub fn cause(&self) -> FailureCause {
        self.cause
    }
}

/// Why recovery became terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalReason {
    RetryExhausted,
    AbruptShutdown,
}

/// Durable terminal result returned to the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalResult {
    reason: TerminalReason,
}

impl TerminalResult {
    pub fn reason(&self) -> &TerminalReason {
        &self.reason
    }
}

/// Result of a recovery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPlan {
    previous_epoch: EngineEpoch,
    new_epoch: EngineEpoch,
    form_submission_aborted: bool,
    terminal: bool,
}

impl RecoveryPlan {
    pub fn previous_epoch(&self) -> EngineEpoch {
        self.previous_epoch
    }

    pub fn new_epoch(&self) -> EngineEpoch {
        self.new_epoch
    }

    pub fn form_submission_aborted(&self) -> bool {
        self.form_submission_aborted
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
}

/// Errors returned when recovery fencing or lifecycle rules reject an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    InvalidState {
        operation: &'static str,
        state: RecoveryState,
    },
    StaleEpoch {
        expected: EngineEpoch,
        actual: EngineEpoch,
    },
    StaleGeneration {
        expected: u32,
        actual: u32,
    },
    Terminal {
        reason: TerminalReason,
    },
    JournalConflict,
    Persistence {
        operation: &'static str,
        detail: String,
    },
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidState { operation, state } => {
                write!(formatter, "{operation} rejected in {state:?}")
            }
            Self::StaleEpoch { expected, actual } => {
                write!(
                    formatter,
                    "stale engine epoch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::StaleGeneration { expected, actual } => {
                write!(
                    formatter,
                    "stale generation: expected at least {expected}, got {actual}"
                )
            }
            Self::Terminal { reason } => write!(formatter, "recovery is terminal: {reason:?}"),
            Self::JournalConflict => write!(formatter, "checkpoint journal conflict"),
            Self::Persistence { operation, detail } => {
                write!(formatter, "{operation} failed: {detail}")
            }
        }
    }
}

impl std::error::Error for RecoveryError {}

/// A prepared checkpoint write that is not visible to recovery until committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCheckpoint {
    sequence: u64,
    checkpoint: RecoveryCheckpoint,
}

/// Small transactional journal seam for checkpoint persistence.
///
/// The in-memory implementation models the required commit/abort semantics.
/// A profile-storage backend can persist the same records without changing the
/// recovery state machine; until then, this type must not be presented as
/// crash-safe process persistence.
#[derive(Debug, Default)]
pub struct CheckpointJournal {
    next_sequence: u64,
    pending: Option<PendingCheckpoint>,
    committed: Option<RecoveryCheckpoint>,
}

impl CheckpointJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare(&mut self, checkpoint: RecoveryCheckpoint) -> PendingCheckpoint {
        self.next_sequence = self.next_sequence.saturating_add(1);
        let pending = PendingCheckpoint {
            sequence: self.next_sequence,
            checkpoint,
        };
        self.pending = Some(pending.clone());
        pending
    }

    pub fn commit(&mut self, pending: PendingCheckpoint) -> Result<(), RecoveryError> {
        if self.pending.as_ref().map(|value| value.sequence) != Some(pending.sequence) {
            return Err(RecoveryError::JournalConflict);
        }
        self.committed = Some(pending.checkpoint);
        self.pending = None;
        Ok(())
    }

    pub fn abort(&mut self, pending: PendingCheckpoint) {
        if self.pending.as_ref().map(|value| value.sequence) == Some(pending.sequence) {
            self.pending = None;
        }
    }

    pub fn recover(&self) -> Option<RecoveryCheckpoint> {
        self.committed.clone()
    }
}
/// A durable append-only journal for committed checkpoints.
///
/// Each committed record is flushed with `sync_all`. Recovery accepts the
/// newest complete record and ignores an incomplete final record, modelling a
/// process interruption during a write without replacing the last commit.
#[derive(Debug)]
pub struct DurableCheckpointJournal {
    path: std::path::PathBuf,
    next_sequence: u64,
    pending: Option<PendingCheckpoint>,
    committed: Option<RecoveryCheckpoint>,
}

impl DurableCheckpointJournal {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, RecoveryError> {
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
        std::fs::File::open(&journal.path)
            .and_then(|mut file| {
                use std::io::Read;
                file.read_to_string(&mut contents)
            })
            .map_err(|error| RecoveryError::Persistence {
                operation: "read checkpoint journal",
                detail: error.to_string(),
            })?;

        let complete_file = contents.ends_with('\n');
        let lines: Vec<&str> = contents.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let is_torn_final_record = index + 1 == lines.len() && !complete_file;
            match decode_checkpoint_record(line) {
                Ok(Some((sequence, checkpoint))) => {
                    journal.next_sequence = journal.next_sequence.max(sequence);
                    journal.committed = Some(checkpoint);
                }
                Ok(None) if is_torn_final_record => {}
                Ok(None) => {
                    return Err(RecoveryError::Persistence {
                        operation: "parse checkpoint journal",
                        detail: "unsupported checkpoint record".to_string(),
                    });
                }
                Err(error) if is_torn_final_record => {
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(journal)
    }

    pub fn prepare(&mut self, checkpoint: RecoveryCheckpoint) -> PendingCheckpoint {
        self.next_sequence = self.next_sequence.saturating_add(1);
        let pending = PendingCheckpoint {
            sequence: self.next_sequence,
            checkpoint,
        };
        self.pending = Some(pending.clone());
        pending
    }

    pub fn commit(&mut self, pending: PendingCheckpoint) -> Result<(), RecoveryError> {
        if self.pending.as_ref().map(|value| value.sequence) != Some(pending.sequence) {
            return Err(RecoveryError::JournalConflict);
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| RecoveryError::Persistence {
                operation: "create checkpoint journal directory",
                detail: error.to_string(),
            })?;
        }
        let record = encode_checkpoint_record(pending.sequence, &pending.checkpoint)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| RecoveryError::Persistence {
                operation: "open checkpoint journal",
                detail: error.to_string(),
            })?;
        use std::io::Write;
        file.write_all(record.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| RecoveryError::Persistence {
                operation: "flush checkpoint journal",
                detail: error.to_string(),
            })?;

        self.committed = Some(pending.checkpoint);
        self.pending = None;
        Ok(())
    }

    pub fn abort(&mut self, pending: PendingCheckpoint) {
        if self.pending.as_ref().map(|value| value.sequence) == Some(pending.sequence) {
            self.pending = None;
        }
    }

    pub fn recover(&self) -> Result<Option<RecoveryCheckpoint>, RecoveryError> {
        Ok(self.committed.clone())
    }
}

fn encode_checkpoint_record(
    sequence: u64,
    checkpoint: &RecoveryCheckpoint,
) -> Result<String, RecoveryError> {
    let url =
        serde_json::to_string(&checkpoint.url).map_err(|error| RecoveryError::Persistence {
            operation: "encode checkpoint",
            detail: error.to_string(),
        })?;
    Ok(format!(
        "PR028|1|{}|{}|{}|{}|{}\n",
        sequence, checkpoint.epoch.0, checkpoint.generation, checkpoint.form_in_flight, url
    ))
}

fn decode_checkpoint_record(
    line: &str,
) -> Result<Option<(u64, RecoveryCheckpoint)>, RecoveryError> {
    let mut fields = line.splitn(7, '|');
    if fields.next() != Some("PR028") {
        return Ok(None);
    }
    if fields.next() != Some("1") {
        return Err(RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: "unsupported checkpoint schema".to_string(),
        });
    }
    let sequence = parse_journal_number(fields.next(), "sequence")?;
    let epoch = parse_journal_number(fields.next(), "epoch")?;
    let generation = parse_journal_number(fields.next(), "generation")? as u32;
    let form_in_flight = fields
        .next()
        .ok_or_else(|| RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: "missing form flag".to_string(),
        })?
        .parse::<bool>()
        .map_err(|error| RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: error.to_string(),
        })?;
    let url = serde_json::from_str::<Option<String>>(fields.next().unwrap_or("null")).map_err(
        |error| RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: error.to_string(),
        },
    )?;

    Ok(Some((
        sequence,
        RecoveryCheckpoint::new(EngineEpoch(epoch), generation, url, form_in_flight),
    )))
}

fn parse_journal_number(value: Option<&str>, field: &'static str) -> Result<u64, RecoveryError> {
    value
        .ok_or_else(|| RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: format!("missing {field}"),
        })?
        .parse::<u64>()
        .map_err(|error| RecoveryError::Persistence {
            operation: "parse checkpoint journal",
            detail: error.to_string(),
        })
}

/// Owns crash/hang recovery state for one browser tab/engine host.
#[derive(Debug)]
pub struct RecoveryCoordinator {
    max_restart_attempts: u32,
    restart_attempts: u32,
    epoch: EngineEpoch,
    generation: u32,
    state: RecoveryState,
    checkpoint: Option<RecoveryCheckpoint>,
    journal: CheckpointJournal,
    diagnostics: Option<CrashDiagnostics>,
    terminal_result: Option<TerminalResult>,
}

impl RecoveryCoordinator {
    pub fn new(max_restart_attempts: u32) -> Self {
        Self {
            max_restart_attempts,
            restart_attempts: 0,
            epoch: EngineEpoch::initial(),
            generation: 0,
            state: RecoveryState::Running,
            checkpoint: None,
            journal: CheckpointJournal::new(),
            diagnostics: None,
            terminal_result: None,
        }
    }

    pub fn state(&self) -> RecoveryState {
        self.state
    }

    pub fn epoch(&self) -> EngineEpoch {
        self.epoch
    }

    pub fn checkpoint(&self) -> Option<&RecoveryCheckpoint> {
        self.checkpoint.as_ref()
    }

    pub fn terminal_result(&self) -> Option<&TerminalResult> {
        self.terminal_result.as_ref()
    }

    /// Record a crash with a fixed redacted detail marker.
    pub fn record_crash(
        &mut self,
        cause: FailureCause,
        _detail: &str,
    ) -> Result<CrashDiagnostics, RecoveryError> {
        self.ensure_not_terminal("record_crash")?;
        self.state = RecoveryState::Crashed;
        let diagnostics = CrashDiagnostics {
            cause,
            redacted_detail: "[REDACTED]",
        };
        self.diagnostics = Some(diagnostics.clone());
        Ok(diagnostics)
    }

    /// Record a watchdog timeout without retaining page or form details.
    pub fn record_hang(&mut self, _detail: &str) -> Result<CrashDiagnostics, RecoveryError> {
        self.ensure_not_terminal("record_hang")?;
        self.state = RecoveryState::Hung;
        let diagnostics = CrashDiagnostics {
            cause: FailureCause::WatchdogTimeout,
            redacted_detail: "[REDACTED]",
        };
        self.diagnostics = Some(diagnostics.clone());
        Ok(diagnostics)
    }

    pub fn save_checkpoint(&mut self, checkpoint: RecoveryCheckpoint) -> Result<(), RecoveryError> {
        self.ensure_not_terminal("save_checkpoint")?;
        self.ensure_epoch(checkpoint.epoch)?;
        if checkpoint.generation < self.generation {
            return Err(RecoveryError::StaleGeneration {
                expected: self.generation,
                actual: checkpoint.generation,
            });
        }
        let pending = self.journal.prepare(checkpoint.clone());
        self.journal.commit(pending)?;
        self.generation = checkpoint.generation;
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    /// Accept an event only when its epoch and generation are current.
    pub fn accept_event(&mut self, fence: EventFence) -> Result<(), RecoveryError> {
        self.ensure_not_terminal("accept_event")?;
        self.ensure_epoch(fence.epoch)?;
        if fence.generation < self.generation {
            return Err(RecoveryError::StaleGeneration {
                expected: self.generation,
                actual: fence.generation,
            });
        }
        self.generation = fence.generation;
        Ok(())
    }

    /// Restart once, advancing the engine epoch and aborting form replay.
    pub fn restart(&mut self) -> Result<RecoveryPlan, RecoveryError> {
        if self.state == RecoveryState::Terminal {
            return Err(RecoveryError::Terminal {
                reason: self
                    .terminal_result
                    .map_or(TerminalReason::RetryExhausted, |result| result.reason),
            });
        }
        if !matches!(self.state, RecoveryState::Crashed | RecoveryState::Hung) {
            return Err(RecoveryError::InvalidState {
                operation: "restart",
                state: self.state,
            });
        }

        self.restart_attempts = self.restart_attempts.saturating_add(1);
        let previous_epoch = self.epoch;
        if self.restart_attempts > self.max_restart_attempts {
            self.state = RecoveryState::Terminal;
            self.terminal_result = Some(TerminalResult {
                reason: TerminalReason::RetryExhausted,
            });
            return Ok(RecoveryPlan {
                previous_epoch,
                new_epoch: previous_epoch,
                form_submission_aborted: true,
                terminal: true,
            });
        }

        self.state = RecoveryState::Restarting;
        self.epoch = self.epoch.next();
        self.generation = self.generation.saturating_add(1);
        self.state = RecoveryState::Running;
        Ok(RecoveryPlan {
            previous_epoch,
            new_epoch: self.epoch,
            form_submission_aborted: self
                .checkpoint
                .as_ref()
                .is_some_and(RecoveryCheckpoint::form_in_flight),
            terminal: false,
        })
    }

    /// Enter terminal state while retaining the last valid checkpoint.
    pub fn abrupt_shutdown(&mut self) -> RecoveryPlan {
        let previous_epoch = self.epoch;
        self.state = RecoveryState::Terminal;
        self.terminal_result = Some(TerminalResult {
            reason: TerminalReason::AbruptShutdown,
        });
        RecoveryPlan {
            previous_epoch,
            new_epoch: previous_epoch,
            form_submission_aborted: true,
            terminal: true,
        }
    }

    fn ensure_not_terminal(&self, operation: &'static str) -> Result<(), RecoveryError> {
        if self.state == RecoveryState::Terminal {
            return Err(RecoveryError::InvalidState {
                operation,
                state: self.state,
            });
        }
        Ok(())
    }

    fn ensure_epoch(&self, actual: EngineEpoch) -> Result<(), RecoveryError> {
        if actual != self.epoch {
            return Err(RecoveryError::StaleEpoch {
                expected: self.epoch,
                actual,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(epoch: EngineEpoch, generation: u32, form_in_flight: bool) -> RecoveryCheckpoint {
        RecoveryCheckpoint::new(
            epoch,
            generation,
            Some("https://example.test/checkout".to_string()),
            form_in_flight,
        )
    }

    fn test_checkpoint_path(label: &str) -> std::path::PathBuf {
        let unique = format!(
            "pr028-{}-{}-{label}.journal",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn crash_records_redacted_diagnostics_without_raw_detail() {
        let mut recovery = RecoveryCoordinator::new(3);
        let secret = "Authorization=Bearer top-secret-token";
        let diagnostics = recovery
            .record_crash(FailureCause::Panic, secret)
            .expect("record crash");

        assert_eq!(recovery.state(), RecoveryState::Crashed);
        assert_eq!(diagnostics.cause(), FailureCause::Panic);
        assert!(!format!("{diagnostics:?}").contains(secret));
    }

    #[test]
    fn hang_is_observable_as_watchdog_failure() {
        let mut recovery = RecoveryCoordinator::new(3);
        let diagnostics = recovery
            .record_hang("page URL and form body must not enter diagnostics")
            .expect("record hang");

        assert_eq!(recovery.state(), RecoveryState::Hung);
        assert_eq!(diagnostics.cause(), FailureCause::WatchdogTimeout);
    }

    #[test]
    fn restart_creates_new_epoch_and_aborts_form_resubmission() {
        let mut recovery = RecoveryCoordinator::new(3);
        let old_epoch = recovery.epoch();
        recovery
            .save_checkpoint(checkpoint(old_epoch, 7, true))
            .expect("checkpoint");
        recovery
            .record_crash(FailureCause::OutOfMemory, "redact me")
            .unwrap();

        let plan = recovery.restart().expect("restart plan");
        assert_eq!(plan.previous_epoch(), old_epoch);
        assert!(plan.new_epoch() > old_epoch);
        assert!(plan.form_submission_aborted());
        assert_eq!(recovery.state(), RecoveryState::Running);
        assert_eq!(recovery.epoch(), plan.new_epoch());
    }

    #[test]
    fn stale_event_from_previous_epoch_is_rejected_after_restart() {
        let mut recovery = RecoveryCoordinator::new(3);
        let old_epoch = recovery.epoch();
        recovery
            .record_crash(FailureCause::Panic, "old detail")
            .unwrap();
        recovery.restart().unwrap();

        let error = recovery
            .accept_event(EventFence::new(old_epoch, 1))
            .expect_err("old epoch must be fenced");
        assert!(matches!(error, RecoveryError::StaleEpoch { .. }));
    }

    #[test]
    fn stale_generation_is_rejected_even_in_current_epoch() {
        let mut recovery = RecoveryCoordinator::new(3);
        let epoch = recovery.epoch();
        recovery
            .save_checkpoint(checkpoint(epoch, 9, false))
            .unwrap();

        let error = recovery
            .accept_event(EventFence::new(epoch, 8))
            .expect_err("old generation must be fenced");
        assert!(matches!(error, RecoveryError::StaleGeneration { .. }));
        recovery
            .accept_event(EventFence::new(epoch, 9))
            .expect("current generation accepted");
    }

    #[test]
    fn retry_exhaustion_produces_terminal_result() {
        let mut recovery = RecoveryCoordinator::new(1);
        recovery.record_crash(FailureCause::Panic, "first").unwrap();
        recovery.restart().unwrap();
        recovery
            .record_crash(FailureCause::Panic, "second")
            .unwrap();

        let terminal = recovery.restart().expect("terminal decision");
        assert!(terminal.is_terminal());
        assert_eq!(recovery.state(), RecoveryState::Terminal);
        assert!(recovery.terminal_result().is_some());
        assert!(recovery.restart().is_err());
    }

    #[test]
    fn abrupt_shutdown_preserves_checkpoint_but_aborts_recovery() {
        let mut recovery = RecoveryCoordinator::new(3);
        let epoch = recovery.epoch();
        recovery
            .save_checkpoint(checkpoint(epoch, 4, false))
            .unwrap();
        let terminal = recovery.abrupt_shutdown();

        assert!(terminal.is_terminal());
        assert_eq!(recovery.state(), RecoveryState::Terminal);
        assert!(recovery.checkpoint().is_some());
        assert!(matches!(
            recovery.terminal_result().map(|result| result.reason()),
            Some(TerminalReason::AbruptShutdown)
        ));
    }

    #[test]
    fn checkpoint_journal_recovers_last_committed_write_after_abrupt_write() {
        let mut journal = CheckpointJournal::new();
        let first = checkpoint(EngineEpoch::initial(), 1, false);
        let first_pending = journal.prepare(first);
        journal.commit(first_pending).expect("first commit");

        let pending = journal.prepare(checkpoint(EngineEpoch::initial(), 2, false));
        drop(pending); // abrupt interruption before commit

        assert_eq!(journal.recover().map(|value| value.generation()), Some(1));
    }

    #[test]
    fn checkpoint_journal_abort_does_not_replace_last_commit() {
        let mut journal = CheckpointJournal::new();
        let first_pending = journal.prepare(checkpoint(EngineEpoch::initial(), 1, false));
        journal.commit(first_pending).expect("first commit");
        let pending = journal.prepare(checkpoint(EngineEpoch::initial(), 2, true));

        journal.abort(pending);

        assert_eq!(journal.recover().map(|value| value.generation()), Some(1));
    }

    #[test]
    fn checkpoint_journal_commit_makes_new_checkpoint_recoverable() {
        let mut journal = CheckpointJournal::new();
        let first_pending = journal.prepare(checkpoint(EngineEpoch::initial(), 1, false));
        journal.commit(first_pending).expect("first commit");
        let pending = journal.prepare(checkpoint(EngineEpoch::initial(), 2, true));
        journal.commit(pending).expect("second commit");

        let recovered = journal.recover().expect("committed checkpoint");
        assert_eq!(recovered.generation(), 2);
        assert!(recovered.form_in_flight());
    }

    #[test]
    fn durable_checkpoint_journal_survives_reopen() {
        let path = test_checkpoint_path("reopen");
        let mut journal = DurableCheckpointJournal::open(&path).expect("open journal");
        let pending = journal.prepare(checkpoint(EngineEpoch::initial(), 11, false));
        journal.commit(pending).expect("durable commit");
        drop(journal);

        let reopened = DurableCheckpointJournal::open(&path).expect("reopen journal");
        let recovered = reopened
            .recover()
            .expect("read journal")
            .expect("committed checkpoint");
        assert_eq!(recovered.generation(), 11);
        std::fs::remove_file(path).expect("cleanup journal");
    }

    #[test]
    fn durable_checkpoint_journal_keeps_last_commit_after_uncommitted_write() {
        let path = test_checkpoint_path("atomic");
        let mut journal = DurableCheckpointJournal::open(&path).expect("open journal");
        let first = journal.prepare(checkpoint(EngineEpoch::initial(), 3, false));
        journal.commit(first).expect("first commit");
        let uncommitted = journal.prepare(checkpoint(EngineEpoch::initial(), 4, true));
        drop(uncommitted);
        drop(journal);

        let reopened = DurableCheckpointJournal::open(&path).expect("reopen journal");
        assert_eq!(
            reopened
                .recover()
                .expect("read journal")
                .map(|value| value.generation()),
            Some(3)
        );
        std::fs::remove_file(path).expect("cleanup journal");
    }

    #[test]
    fn checkpoint_from_old_epoch_is_rejected() {
        let mut recovery = RecoveryCoordinator::new(3);
        recovery
            .record_crash(FailureCause::Panic, "detail")
            .unwrap();
        recovery.restart().unwrap();

        let old = checkpoint(EngineEpoch::initial(), 1, false);
        let error = recovery.save_checkpoint(old).expect_err("old checkpoint");
        assert!(matches!(error, RecoveryError::StaleEpoch { .. }));
    }
}
