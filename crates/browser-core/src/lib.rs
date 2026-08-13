#![forbid(unsafe_code)]

//! Browser core — the central state owner and lifecycle manager.
//!
//! The core actor owns browser state (tab list, profiles, navigation state),
//! dispatches commands to the engine host, and enforces the runtime lifecycle
//! contract defined in ADR-005 and `docs/contracts/runtime-lifecycle.md`.

pub mod engine_host;
pub mod lifecycle;
pub mod navigation;

pub const PACKAGE_NAME: &str = "browser-core";

#[cfg(test)]
mod tests {
    #[test]
    fn package_name_is_stable() {
        assert_eq!(super::PACKAGE_NAME, "browser-core");
    }
}
