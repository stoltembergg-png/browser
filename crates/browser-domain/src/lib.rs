#![forbid(unsafe_code)]

/// Stable identifier types will be added by a later domain slice.
pub const PACKAGE_NAME: &str = "browser-domain";

#[cfg(test)]
mod tests {
    #[test]
    fn package_name_is_stable() {
        assert_eq!(super::PACKAGE_NAME, "browser-domain");
    }
}
