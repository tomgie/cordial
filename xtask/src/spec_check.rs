//! Compares the endpoints of `cordial-http` with the Discord OpenAPI specification.
//!
//! The specification is at <https://github.com/discord/discord-api-spec> (`specs/openapi.json`).

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

const ENDPOINTS_DIR: &str = "crates/cordial-http/src/endpoints";
const METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];

/// One operation: a method and a normalized path.
type Operation = (String, String);

/// Runs the check. Gives the number of operations in the specification that are missing.
pub(crate) fn run(spec_path: &Path) -> Result<usize, Box<dyn Error>> {
    let spec_text = fs::read_to_string(spec_path)
        .map_err(|error| format!("cannot read {}: {error}", spec_path.display()))?;
    let spec: serde_json::Value = serde_json::from_str(&spec_text)?;

    let specified = specified_operations(&spec)?;
    let implemented = implemented_operations(Path::new(ENDPOINTS_DIR))?;

    let missing: Vec<&Operation> = specified.difference(&implemented).collect();
    let extra: Vec<&Operation> = implemented.difference(&specified).collect();

    println!(
        "Specification: {} operations. Implemented: {}. Missing: {}. Not in the specification: {}.",
        specified.len(),
        implemented.len(),
        missing.len(),
        extra.len()
    );
    if !missing.is_empty() {
        println!("\nMissing:");
        for (method, path) in &missing {
            println!("  {method:<6} {path}");
        }
    }
    if !extra.is_empty() {
        println!("\nNot in the specification (check the path template):");
        for (method, path) in &extra {
            println!("  {method:<6} {path}");
        }
    }

    Ok(missing.len())
}

fn specified_operations(spec: &serde_json::Value) -> Result<BTreeSet<Operation>, Box<dyn Error>> {
    let paths = spec["paths"]
        .as_object()
        .ok_or("the specification has no `paths` object")?;

    let mut operations = BTreeSet::new();
    for (path, item) in paths {
        for method in METHODS {
            if item.get(method).is_some() {
                operations.insert((method.to_uppercase(), normalize(path)));
            }
        }
    }
    Ok(operations)
}

fn implemented_operations(directory: &Path) -> Result<BTreeSet<Operation>, Box<dyn Error>> {
    let mut operations = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)?;
            operations.extend(routes_in_source(&source));
        }
    }
    Ok(operations)
}

/// Finds `Method::X,` followed by a string literal (the template) in the source.
fn routes_in_source(source: &str) -> Vec<Operation> {
    let mut found = Vec::new();
    let mut rest = source;

    while let Some(start) = rest.find("Method::") {
        rest = &rest[start + "Method::".len()..];
        let name_end = rest
            .find(|character: char| !character.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let method = rest[..name_end].to_uppercase();
        let after_name = rest[name_end..].trim_start();

        let Some(after_comma) = after_name.strip_prefix(',') else {
            continue;
        };
        let after_comma = after_comma.trim_start();
        let Some(literal) = after_comma.strip_prefix('"') else {
            continue;
        };
        let Some(end) = literal.find('"') else {
            continue;
        };
        let template = &literal[..end];
        if template.starts_with('/') && METHODS.contains(&method.to_lowercase().as_str()) {
            found.push((method, normalize(template)));
        }
    }
    found
}

/// Replaces each `{name}` in a path with `{}`, so that parameter names do not matter.
fn normalize(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len());
    let mut in_parameter = false;
    for character in path.chars() {
        match character {
            '{' => {
                in_parameter = true;
                normalized.push_str("{}");
            }
            '}' => in_parameter = false,
            _ if in_parameter => {}
            _ => normalized.push(character),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_parameters() {
        assert_eq!(
            normalize("/channels/{channel_id}/messages/{message.id}"),
            "/channels/{}/messages/{}"
        );
    }

    #[test]
    fn finds_routes_in_source() {
        let source = r#"
            Route::new(
                Method::Get,
                "/channels/{channel_id}",
                format!("/channels/{}", self.channel_id),
            )
            let x = Method::Post;
            Route::new(Method::Delete, "/webhooks/{webhook_id}", path, major)
        "#;

        let routes = routes_in_source(source);

        assert_eq!(
            routes,
            vec![
                ("GET".to_owned(), "/channels/{}".to_owned()),
                ("DELETE".to_owned(), "/webhooks/{}".to_owned()),
            ]
        );
    }
}
