#![forbid(unsafe_code)]

pub const PACKAGE_NAME: &str = "test-support";

#[cfg(test)]
mod tests {
    #[test]
    fn support_package_is_dev_facing() {
        assert_eq!(super::PACKAGE_NAME, "test-support");
    }
}
