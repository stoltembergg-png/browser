#![forbid(unsafe_code)]

//! Engine API — engine-neutral contract for the browser's first backend.
//!
//! This module defines the SPI (Service Provider Interface) that the browser
//! core uses to communicate with a web engine. The first backend is Servo
//! (via `servo-engine` adapter, PR-016); a `FakeEngine` exists for contract
//! testing (PR-015).
//!
//! ## Design constraints
//!
//! - The trait is async at the edge but does not force the engine internals
//!   to be `Send + Sync`. The handle represents an actor; real operations
//!   run on the engine's thread.
//! - No Servo types leak through this boundary.
//! - Script evaluation, DevTools, permissions and downloads are NOT in the
//!   first contract — they require capability, threat model and ADR of their own.
//!
//! See ARCHITECTURE.md §6 and ADR-003 for the full rationale.

pub mod contract;
pub mod events;

pub const PACKAGE_NAME: &str = "engine-api";

/// Re-export of the Servo revision examined in the PR-013 spike.
///
/// This is a research snapshot, NOT an approved dependency. The actual Servo
/// crate is not in the workspace. The revision is pinned here for traceability
/// and will be validated when PR-016 implements the adapter.
pub const SERVO_SPIKE_REVISION: &str = "859bd5edd60c0fb162a1f73c083a23e55474faf7";

#[cfg(test)]
mod tests {
    #[test]
    fn package_name_is_stable() {
        assert_eq!(super::PACKAGE_NAME, "engine-api");
    }

    #[test]
    fn servo_spike_revision_is_sha_like() {
        let rev = super::SERVO_SPIKE_REVISION;
        assert_eq!(rev.len(), 40, "SHA-1 hash must be 40 hex characters");
        assert!(
            rev.chars().all(|c| c.is_ascii_hexdigit()),
            "revision must be hex"
        );
    }
}
