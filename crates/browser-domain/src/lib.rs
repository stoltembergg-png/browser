#![forbid(unsafe_code)]

/// Pure browser-domain state and value objects.
pub const PACKAGE_NAME: &str = "browser-domain";

pub mod ids;
pub mod profile;
pub mod session;
pub mod tab;
pub mod ui;

#[cfg(test)]
mod tests {
    #[test]
    fn package_name_is_stable() {
        assert_eq!(super::PACKAGE_NAME, "browser-domain");
    }
}
