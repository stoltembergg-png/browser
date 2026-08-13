use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::process::Command;

fn parse_workspace_packages(metadata: &str) -> Result<BTreeSet<String>, String> {
    let members = metadata
        .split_once("\"workspace_members\":[")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(members, _)| members)
        .ok_or_else(|| "cargo metadata is missing workspace members".to_string())?;
    let mut names = BTreeSet::new();
    for member in members.split(',') {
        let member = member.trim().trim_matches('"');
        let Some((path, suffix)) = member.rsplit_once('#') else {
            return Err("invalid cargo metadata workspace member".to_string());
        };
        let name = suffix.split_once('@').map_or_else(
            || path.rsplit('/').next().unwrap_or_default(),
            |(name, _)| name,
        );
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    if names.is_empty() {
        return Err("cargo metadata contains no packages".to_string());
    }
    Ok(names)
}

fn parse_m0_packages(graph: &str) -> Result<BTreeSet<String>, String> {
    let section = graph
        .split_once("  M0:")
        .map(|(_, rest)| rest)
        .ok_or_else(|| "architecture graph is missing M0".to_string())?;
    let packages = section
        .split_once("    packages:")
        .map(|(_, rest)| rest)
        .ok_or_else(|| "architecture graph is missing M0 packages".to_string())?
        .lines()
        .map(str::trim)
        .skip_while(|line| line.is_empty())
        .take_while(|line| line.starts_with("- "))
        .map(|line| line.trim_start_matches("- ").trim().to_string())
        .collect::<BTreeSet<_>>();
    if packages.is_empty() {
        return Err("architecture graph has no M0 packages".to_string());
    }
    Ok(packages)
}

fn validate_m0(metadata: &str, graph: &str) -> Result<(), String> {
    let actual = parse_workspace_packages(metadata)?;
    let expected = parse_m0_packages(graph)?;
    if actual != expected {
        return Err(format!(
            "M0 package mismatch: actual={actual:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

fn run_check() -> Result<(), String> {
    let metadata = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !metadata.status.success() {
        return Err("cargo metadata failed".to_string());
    }
    let graph = fs::read_to_string("docs/architecture-graph.yaml")
        .map_err(|error| format!("failed to read architecture graph: {error}"))?;
    validate_m0(&String::from_utf8_lossy(&metadata.stdout), &graph)
}

fn main() {
    if let Some("architecture-check") = env::args().nth(1).as_deref() {
        match run_check() {
            Ok(()) => println!("architecture-check: PASS (M0 packages match cargo metadata)"),
            Err(error) => {
                eprintln!("architecture-check: FAIL: {error}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("usage: cargo run -p xtask -- architecture-check");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::validate_m0;

    #[test]
    fn accepts_exact_m0_package_set() {
        let metadata = "{\"workspace_members\":[\"path#browser-domain@0.1.0\",\"path#browser-core@0.1.0\",\"path#engine-api@0.1.0\",\"path#test-support@0.1.0\",\"path#xtask@0.1.0\"]}";
        let graph = "phases:\n  M0:\n    packages:\n      - browser-domain\n      - browser-core\n      - engine-api\n      - test-support\n      - xtask\n";
        assert_eq!(validate_m0(metadata, graph), Ok(()));
    }

    #[test]
    fn rejects_undeclared_package() {
        let metadata =
            "{\"workspace_members\":[\"path#browser-domain@0.1.0\",\"path#unexpected@0.1.0\"]}";
        let graph = "phases:\n  M0:\n    packages:\n      - browser-domain\n";
        let error = validate_m0(metadata, graph).expect_err("extra package must fail");
        assert!(error.contains("unexpected"));
    }

    #[test]
    fn rejects_missing_m0_section() {
        let metadata = "{\"workspace_members\":[\"path#browser-domain@0.1.0\"]}";
        let error = validate_m0(
            metadata,
            "phases:\n  M1:\n    packages:\n      - browser-domain\n",
        )
        .expect_err("missing M0 must fail");
        assert!(error.contains("M0"));
    }
}
