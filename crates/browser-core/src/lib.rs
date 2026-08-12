#![forbid(unsafe_code)]

pub const PACKAGE_NAME: &str = "browser-core";

#[cfg(test)]
mod tests {
    #[test]
    fn core_has_no_product_behavior_yet() {
        assert_eq!(super::PACKAGE_NAME, "browser-core");
    }
}
