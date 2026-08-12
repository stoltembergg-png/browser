#![forbid(unsafe_code)]

pub const PACKAGE_NAME: &str = "engine-api";

#[cfg(test)]
mod tests {
    #[test]
    fn package_name_is_stable() {
        assert_eq!(super::PACKAGE_NAME, "engine-api");
    }
}
