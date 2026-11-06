//! The release check: one version everywhere, the crate metadata, the changelog, and the
//! publish order.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The result of the check: the problems, and the publish order when there is none.
pub(crate) struct Report {
    pub(crate) problems: Vec<String>,
    pub(crate) order: Vec<String>,
    pub(crate) version: String,
}

pub(crate) fn run(root: &Path) -> Result<Report, String> {
    let manifest = read(&root.join("Cargo.toml"))?;
    let version = workspace_version(&manifest)
        .ok_or("the root Cargo.toml has no workspace.package.version")?;
    let mut problems = Vec::new();

    check_pinned_versions(&manifest, &version, &mut problems);

    let crates = crate_manifests(root)?;
    let mut dependencies = BTreeMap::new();
    for (name, text) in &crates {
        check_metadata(name, text, &mut problems);
        dependencies.insert(name.clone(), internal_dependencies(text));
    }

    let changelog = read(&root.join("CHANGELOG.md")).unwrap_or_default();
    if !changelog.contains(&format!("## {version}")) {
        problems.push(format!("CHANGELOG.md has no section `## {version}`"));
    }

    let order = publish_order(&dependencies).unwrap_or_else(|cycle| {
        problems.push(format!("the crates have a dependency cycle: {cycle}"));
        Vec::new()
    });
    Ok(Report {
        problems,
        order,
        version,
    })
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// Gives the `version` of the `[workspace.package]` table.
fn workspace_version(manifest: &str) -> Option<String> {
    let mut in_table = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == "[workspace.package]";
            continue;
        }
        if in_table && let Some(value) = line.strip_prefix("version = ") {
            return Some(value.trim_matches('"').to_owned());
        }
    }
    None
}

/// Each `cordial-*` entry of `[workspace.dependencies]` must pin `=<version>`.
fn check_pinned_versions(manifest: &str, version: &str, problems: &mut Vec<String>) {
    let expected = format!("version = \"={version}\"");
    for line in manifest.lines().filter(|line| line.starts_with("cordial")) {
        if !line.contains(&expected) {
            problems.push(format!(
                "the root Cargo.toml entry does not pin ={version}: {line}"
            ));
        }
    }
}

/// Gives the name and text of each published crate manifest under `crates/`.
fn crate_manifests(root: &Path) -> Result<Vec<(String, String)>, String> {
    let directory = root.join("crates");
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;

    let mut crates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join("Cargo.toml");
        if !path.exists() {
            continue;
        }
        let text = read(&path)?;
        let name = text
            .lines()
            .find_map(|line| line.strip_prefix("name = "))
            .map(|name| name.trim_matches('"').to_owned())
            .ok_or_else(|| format!("{} has no package name", path.display()))?;
        if !text.contains("publish = false") {
            crates.push((name, text));
        }
    }
    crates.sort();
    Ok(crates)
}

fn check_metadata(name: &str, text: &str, problems: &mut Vec<String>) {
    let required = [
        ("version.workspace = true", "the workspace version"),
        ("license.workspace = true", "the workspace license"),
        ("description = ", "a description"),
    ];
    for (needle, what) in required {
        if !text.contains(needle) {
            problems.push(format!("{name} does not use {what}"));
        }
    }
}

/// Gives the `cordial-*` crates in `[dependencies]` (not dev-dependencies, which do not
/// change the publish order).
fn internal_dependencies(text: &str) -> BTreeSet<String> {
    let mut in_dependencies = false;
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if in_dependencies && line.starts_with("cordial") {
            let name: String = line
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '-')
                .collect();
            names.insert(name);
        }
    }
    names
}

/// Sorts the crates so that each crate comes after its dependencies.
fn publish_order(dependencies: &BTreeMap<String, BTreeSet<String>>) -> Result<Vec<String>, String> {
    let mut order = Vec::new();
    let mut done = BTreeSet::new();

    while done.len() < dependencies.len() {
        let ready: Vec<String> = dependencies
            .iter()
            .filter(|(name, needs)| {
                !done.contains(*name) && needs.iter().all(|need| done.contains(need))
            })
            .map(|(name, _)| name.clone())
            .collect();
        if ready.is_empty() {
            let rest: Vec<&String> = dependencies
                .keys()
                .filter(|name| !done.contains(*name))
                .collect();
            return Err(format!("{rest:?}"));
        }
        for name in ready {
            done.insert(name.clone());
            order.push(name);
        }
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_crates_after_their_dependencies() {
        let mut dependencies = BTreeMap::new();
        dependencies.insert(
            "cordial".to_owned(),
            BTreeSet::from(["cordial-http".to_owned()]),
        );
        dependencies.insert(
            "cordial-http".to_owned(),
            BTreeSet::from(["cordial-model".to_owned()]),
        );
        dependencies.insert("cordial-model".to_owned(), BTreeSet::new());

        let order = publish_order(&dependencies).unwrap();

        assert_eq!(order, ["cordial-model", "cordial-http", "cordial"]);
    }

    #[test]
    fn reads_the_workspace_version() {
        let manifest = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"1.2.3\"\n";

        assert_eq!(workspace_version(manifest).as_deref(), Some("1.2.3"));
    }
}
