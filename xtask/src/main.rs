use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::process::Command;

fn parse_metadata_packages(metadata: &str) -> Result<BTreeSet<String>, String> {
    let package_section = metadata
        .split_once("\"packages\":[")
        .map(|(_, rest)| rest)
        .ok_or_else(|| "cargo metadata is missing packages".to_string())?;
    let packages = package_section
        .split_once("],\"workspace_members\"")
        .map_or(package_section, |(packages, _)| packages);
    let mut names = BTreeSet::new();
    for object in packages.split('{').skip(1) {
        let Some((_, rest)) = object.split_once("\"name\":\"") else {
            continue;
        };
        let Some((name, _)) = rest.split_once('"') else {
            return Err("invalid cargo metadata package object".to_string());
        };
        if object.contains("\"version\":\"") {
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
    let actual = parse_metadata_packages(metadata)?;
    let expected = parse_m0_packages(graph)?;
    let allowed_additions = graph
        .split_once("  M1:")
        .and_then(|(_, rest)| rest.split_once("    adds:"))
        .map(|(_, adds)| {
            adds.lines()
                .map(str::trim)
                .skip_while(|line| line.is_empty())
                .take_while(|line| line.starts_with("- "))
                .map(|line| line.trim_start_matches("- ").trim().to_string())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if !actual.is_superset(&expected)
        || actual
            .difference(&expected)
            .any(|package| !allowed_additions.contains(package))
    {
        return Err(format!(
            "M0 package mismatch: actual={actual:?}, expected={expected:?}"
        ));
    }
    Ok(())
}

fn run_check() -> Result<(), String> {
    let metadata = Command::new("cargo")
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
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
        let metadata = "{\"packages\":[{\"name\":\"browser-domain\",\"version\":\"0.1.0\"},{\"name\":\"browser-core\",\"version\":\"0.1.0\"},{\"name\":\"engine-api\",\"version\":\"0.1.0\"},{\"name\":\"test-support\",\"version\":\"0.1.0\"},{\"name\":\"xtask\",\"version\":\"0.1.0\"}]}";
        let graph = "phases:\n  M0:\n    packages:\n      - browser-domain\n      - browser-core\n      - engine-api\n      - test-support\n      - xtask\n";
        assert_eq!(validate_m0(metadata, graph), Ok(()));
    }

    #[test]
    fn rejects_undeclared_package() {
        let metadata = "{\"packages\":[{\"name\":\"browser-domain\",\"version\":\"0.1.0\"},{\"name\":\"unexpected\",\"version\":\"0.1.0\"}]}";
        let graph = "phases:\n  M0:\n    packages:\n      - browser-domain\n";
        let error = validate_m0(metadata, graph).expect_err("extra package must fail");
        assert!(error.contains("unexpected"));
    }

    #[test]
    fn rejects_missing_m0_section() {
        let metadata = "{\"packages\":[{\"name\":\"browser-domain\",\"version\":\"0.1.0\"}]}";
        let error = validate_m0(
            metadata,
            "phases:\n  M1:\n    packages:\n      - browser-domain\n",
        )
        .expect_err("missing M0 must fail");
        assert!(error.contains("M0"));
    }
}
