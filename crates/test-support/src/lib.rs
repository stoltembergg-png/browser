#![forbid(unsafe_code)]

//! Engine-neutral test support: shared fixtures and the MVP smoke harness
//! with bound evidence (PR-029).

pub mod smoke;

pub const PACKAGE_NAME: &str = "test-support";

#[cfg(test)]
mod tests {
    #[test]
    fn support_package_is_dev_facing() {
        assert_eq!(super::PACKAGE_NAME, "test-support");
    }
}
