//! Core lifecycle — state machine for the browser core actor.
//!
//! Implements the runtime lifecycle contract from ADR-005 and
//! `docs/contracts/runtime-lifecycle.md`. The core actor transitions
//! between lifecycle states and rejects illegal transitions.
//!
//! ## States
//!
//! ```text
//! Created → Starting → Ready → Navigating → Closing → Exited
//!                        ↘ Failed → Restarting → Ready
//!                        ↘ Crashed → Exited
//! ```
//!
//! Any transition not listed below fails closed.

use engine_api::contract::EngineCommand;
use std::collections::HashSet;

/// The lifecycle state of the browser core actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreState {
    Created,
    Starting,
    Ready,
    Navigating,
    Closing,
    Exited,
    Failed,
    Crashed,
    Restarting,
}

impl CoreState {
    /// Returns `true` if commands can be dispatched in this state.
    pub fn accepts_commands(&self) -> bool {
        matches!(self, CoreState::Ready | CoreState::Navigating)
    }

    /// Returns `true` if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, CoreState::Exited)
    }
}

impl std::fmt::Display for CoreState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Error returned when a state transition is illegal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: CoreState,
    pub to: CoreState,
    pub reason: String,
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal transition: {} → {} ({})",
            self.from, self.to, self.reason
        )
    }
}

impl std::error::Error for IllegalTransition {}

/// Runtime quota parameters from ADR-005.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeQuota {
    pub command_timeout_ms: u32,
    pub queue_capacity: usize,
    pub shutdown_drain_timeout_ms: u32,
    pub restart_max_attempts: u32,
    pub restart_backoff_ms: u32,
    pub input_coalesce: bool,
}

impl Default for RuntimeQuota {
    fn default() -> Self {
        Self {
            command_timeout_ms: 30000,
            queue_capacity: 256,
            shutdown_drain_timeout_ms: 5000,
            restart_max_attempts: 3,
            restart_backoff_ms: 1000,
            input_coalesce: true,
        }
    }
}

/// Commands that must never be silently discarded (reliable delivery).
pub fn is_reliable_command(command: &EngineCommand) -> bool {
    matches!(
        command,
        EngineCommand::Navigate { .. } | EngineCommand::Reload | EngineCommand::Shutdown
    )
}

/// The core actor state machine.
#[derive(Debug)]
pub struct CoreActor {
    state: CoreState,
    quota: RuntimeQuota,
    restart_attempts: u32,
    /// Seen request IDs for duplicate detection.
    seen_requests: HashSet<String>,
    /// Pending commands awaiting dispatch (bounded by quota.queue_capacity).
    pending_queue: Vec<String>,
}

impl CoreActor {
    pub fn new() -> Self {
        Self {
            state: CoreState::Created,
            quota: RuntimeQuota::default(),
            restart_attempts: 0,
            seen_requests: HashSet::new(),
            pending_queue: Vec::new(),
        }
    }

    pub fn with_quota(quota: RuntimeQuota) -> Self {
        Self {
            state: CoreState::Created,
            quota,
            restart_attempts: 0,
            seen_requests: HashSet::new(),
            pending_queue: Vec::new(),
        }
    }

    pub fn state(&self) -> CoreState {
        self.state
    }

    pub fn quota(&self) -> &RuntimeQuota {
        &self.quota
    }

    /// Attempt a state transition. Returns `Err` if illegal.
    pub fn transition(&mut self, to: CoreState) -> Result<(), IllegalTransition> {
        let from = self.state;

        let allowed = match (from, to) {
            (CoreState::Created, CoreState::Starting) => true,
            (CoreState::Starting, CoreState::Ready) => true,
            (CoreState::Starting, CoreState::Failed) => true,
            (CoreState::Ready, CoreState::Navigating) => true,
            (CoreState::Navigating, CoreState::Ready) => true,
            (CoreState::Navigating, CoreState::Failed) => true,
            (CoreState::Ready, CoreState::Closing) => true,
            (CoreState::Navigating, CoreState::Closing) => true,
            (CoreState::Closing, CoreState::Exited) => true,
            (CoreState::Ready, CoreState::Crashed) => true,
            (CoreState::Navigating, CoreState::Crashed) => true,
            (CoreState::Failed, CoreState::Restarting) => true,
            (CoreState::Failed, CoreState::Closing) => true,
            (CoreState::Crashed, CoreState::Restarting) => true,
            (CoreState::Crashed, CoreState::Exited) => true,
            (CoreState::Restarting, CoreState::Ready) => true,
            (CoreState::Restarting, CoreState::Failed) => true,
            // Idempotent: same state is always OK
            (_, _) if from == to => true,
            _ => false,
        };

        if !allowed {
            return Err(IllegalTransition {
                from,
                to,
                reason: "transition not in lifecycle table".to_string(),
            });
        }

        // Track restart attempts
        if to == CoreState::Restarting {
            self.restart_attempts += 1;
            if self.restart_attempts > self.quota.restart_max_attempts {
                // Exceeding max restart attempts transitions to Exited.
                self.state = CoreState::Exited;
                return Ok(());
            }
        }

        if to == CoreState::Ready && from == CoreState::Restarting {
            self.restart_attempts = 0;
        }

        self.state = to;
        Ok(())
    }

    /// Check if a command can be dispatched in the current state.
    pub fn can_dispatch(&self) -> bool {
        self.state.accepts_commands()
    }

    /// Check if a request ID is a duplicate. Returns `true` if already seen.
    /// If new, records it and returns `false`.
    pub fn check_duplicate(&mut self, request_id: &str) -> bool {
        if self.seen_requests.contains(request_id) {
            return true;
        }
        self.seen_requests.insert(request_id.to_string());
        false
    }

    /// Enqueue a pending command. Returns `Err` if queue is saturated.
    pub fn enqueue(&mut self, request_id: &str) -> Result<(), &'static str> {
        if self.pending_queue.len() >= self.quota.queue_capacity {
            return Err("queue saturated");
        }
        self.pending_queue.push(request_id.to_string());
        Ok(())
    }

    /// Dequeue a completed command.
    pub fn dequeue(&mut self, request_id: &str) {
        self.pending_queue.retain(|r| r != request_id);
    }

    /// Returns the number of pending commands.
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }

    /// Returns `true` if the queue is at capacity (backpressure signal).
    pub fn is_queue_saturated(&self) -> bool {
        self.pending_queue.len() >= self.quota.queue_capacity
    }

    /// Shutdown: transition to Closing, drain all pending, then Exited.
    pub fn shutdown(&mut self) -> Result<(), IllegalTransition> {
        self.transition(CoreState::Closing)?;
        // Cancel all pending commands
        self.pending_queue.clear();
        self.transition(CoreState::Exited)?;
        Ok(())
    }
}

impl Default for CoreActor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Legal transitions ---

    #[test]
    fn created_to_starting() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        assert_eq!(actor.state(), CoreState::Starting);
    }

    #[test]
    fn starting_to_ready() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        assert_eq!(actor.state(), CoreState::Ready);
    }

    #[test]
    fn ready_to_navigating_and_back() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Navigating).unwrap();
        assert_eq!(actor.state(), CoreState::Navigating);
        actor.transition(CoreState::Ready).unwrap();
        assert_eq!(actor.state(), CoreState::Ready);
    }

    #[test]
    fn full_shutdown_sequence() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Navigating).unwrap();
        actor.transition(CoreState::Closing).unwrap();
        actor.transition(CoreState::Exited).unwrap();
        assert!(actor.state().is_terminal());
    }

    #[test]
    fn sd_shutdown_helper() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.shutdown().unwrap();
        assert_eq!(actor.state(), CoreState::Exited);
    }

    // --- Illegal transitions ---

    #[test]
    fn created_to_ready_illegal() {
        let mut actor = CoreActor::new();
        assert!(actor.transition(CoreState::Ready).is_err());
    }

    #[test]
    fn exited_to_anything_illegal() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Closing).unwrap();
        actor.transition(CoreState::Exited).unwrap();
        assert!(actor.transition(CoreState::Ready).is_err());
        assert!(actor.transition(CoreState::Starting).is_err());
    }

    #[test]
    fn navigating_to_starting_illegal() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Navigating).unwrap();
        assert!(actor.transition(CoreState::Starting).is_err());
    }

    // --- Command dispatch ---

    #[test]
    fn cannot_dispatch_in_created() {
        let actor = CoreActor::new();
        assert!(!actor.can_dispatch());
    }

    #[test]
    fn can_dispatch_in_ready() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        assert!(actor.can_dispatch());
    }

    #[test]
    fn can_dispatch_in_navigating() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Navigating).unwrap();
        assert!(actor.can_dispatch());
    }

    #[test]
    fn cannot_dispatch_in_exited() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.shutdown().unwrap();
        assert!(!actor.can_dispatch());
    }

    // --- Duplicate detection ---

    #[test]
    fn duplicate_request_id_detected() {
        let mut actor = CoreActor::new();
        assert!(!actor.check_duplicate("req-1")); // First time: not duplicate
        assert!(actor.check_duplicate("req-1")); // Second time: duplicate
    }

    // --- Queue backpressure ---

    #[test]
    fn enqueue_until_saturated() {
        let mut actor = CoreActor::with_quota(RuntimeQuota {
            queue_capacity: 2,
            ..Default::default()
        });
        assert!(actor.enqueue("r1").is_ok());
        assert!(actor.enqueue("r2").is_ok());
        assert!(actor.enqueue("r3").is_err()); // Saturated
        assert!(actor.is_queue_saturated());
        assert_eq!(actor.pending_count(), 2);
    }

    #[test]
    fn dequeue_reduces_pending() {
        let mut actor = CoreActor::with_quota(RuntimeQuota {
            queue_capacity: 3,
            ..Default::default()
        });
        actor.enqueue("r1").unwrap();
        actor.enqueue("r2").unwrap();
        actor.dequeue("r1");
        assert_eq!(actor.pending_count(), 1);
    }

    #[test]
    fn shutdown_clears_pending() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.enqueue("r1").unwrap();
        actor.enqueue("r2").unwrap();
        actor.shutdown().unwrap();
        assert_eq!(actor.pending_count(), 0);
    }

    // --- Crash and restart ---

    #[test]
    fn crash_from_ready() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Crashed).unwrap();
        assert_eq!(actor.state(), CoreState::Crashed);
    }

    #[test]
    fn restart_after_crash() {
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Crashed).unwrap();
        actor.transition(CoreState::Restarting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        assert_eq!(actor.state(), CoreState::Ready);
    }

    #[test]
    fn restart_attempts_exhausted() {
        let mut actor = CoreActor::with_quota(RuntimeQuota {
            restart_max_attempts: 1,
            ..Default::default()
        });
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Crashed).unwrap();
        actor.transition(CoreState::Restarting).unwrap(); // attempt 1
        actor.transition(CoreState::Failed).unwrap();
        // Second restart attempt should exceed max
        actor.transition(CoreState::Restarting).unwrap(); // attempt 2 — exceeds max 1
        assert_eq!(actor.state(), CoreState::Exited);
    }

    // --- Reliable commands ---

    #[test]
    fn navigate_is_reliable() {
        assert!(is_reliable_command(&EngineCommand::Navigate {
            url: "x".into()
        }));
    }

    #[test]
    fn reload_is_reliable() {
        assert!(is_reliable_command(&EngineCommand::Reload));
    }

    #[test]
    fn shutdown_is_reliable() {
        assert!(is_reliable_command(&EngineCommand::Shutdown));
    }

    #[test]
    fn stop_is_not_reliable() {
        assert!(!is_reliable_command(&EngineCommand::Stop));
    }

    #[test]
    fn set_viewport_is_not_reliable() {
        assert!(!is_reliable_command(&EngineCommand::SetViewport {
            width: 1,
            height: 1
        }));
    }

    // --- Interleavings ---

    #[test]
    fn close_start_crash_interleaving() {
        // Simulate: start → ready → navigating → crash → restart → ready → shutdown
        let mut actor = CoreActor::new();
        actor.transition(CoreState::Starting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.transition(CoreState::Navigating).unwrap();
        actor.transition(CoreState::Crashed).unwrap();
        actor.transition(CoreState::Restarting).unwrap();
        actor.transition(CoreState::Ready).unwrap();
        actor.shutdown().unwrap();
        assert!(actor.state().is_terminal());
    }

    // --- RuntimeQuota defaults ---

    #[test]
    fn quota_defaults_match_adr_005() {
        let quota = RuntimeQuota::default();
        assert_eq!(quota.command_timeout_ms, 30000);
        assert_eq!(quota.queue_capacity, 256);
        assert_eq!(quota.shutdown_drain_timeout_ms, 5000);
        assert_eq!(quota.restart_max_attempts, 3);
        assert_eq!(quota.restart_backoff_ms, 1000);
        assert!(quota.input_coalesce);
    }
}
