//! Extracts the JSON examples from the Discord docs into test fixtures.
//!
//! The source is the `developers/` folder of a `discord/discord-api-docs` checkout.
//! Each valid `json` code block becomes one file. The file name comes from the nearest heading.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const OUTPUT_DIR: &str = "fixtures/docs";

/// One JSON example found in a docs page.
struct Example {
    heading: String,
    body: String,
}

/// Extracts all examples from `docs_dir` into `fixtures/docs`.
pub(crate) fn run(docs_dir: &Path) -> Result<(), Box<dyn Error>> {
    if !docs_dir.is_dir() {
        return Err(format!("not a directory: {}", docs_dir.display()).into());
    }

    let output_root = PathBuf::from(OUTPUT_DIR);
    if output_root.exists() {
        fs::remove_dir_all(&output_root)?;
    }

    let mut pages = Vec::new();
    collect_pages(docs_dir, &mut pages)?;
    pages.sort();

    let mut written = 0;
    let mut skipped = 0;
    for page in &pages {
        let text = fs::read_to_string(page)?;
        let relative = page.strip_prefix(docs_dir)?.with_extension("");
        let page_dir = output_root.join(relative);

        let mut used_names: BTreeMap<String, usize> = BTreeMap::new();
        for example in extract_examples(&text) {
            // Some examples contain comments or "..." and are not valid JSON.
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&example.body) else {
                skipped += 1;
                continue;
            };

            let name = unique_name(&slug(&example.heading), &mut used_names);
            fs::create_dir_all(&page_dir)?;
            let pretty = serde_json::to_string_pretty(&value)?;
            fs::write(page_dir.join(format!("{name}.json")), pretty + "\n")?;
            written += 1;
        }
    }

    println!(
        "Wrote {written} fixtures to {OUTPUT_DIR}. Skipped {skipped} blocks that are not valid JSON."
    );
    Ok(())
}

fn collect_pages(dir: &Path, pages: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pages(&path, pages)?;
        } else if path.extension().is_some_and(|extension| extension == "mdx") {
            pages.push(path);
        }
    }
    Ok(())
}

fn extract_examples(text: &str) -> Vec<Example> {
    let mut examples = Vec::new();
    let mut heading = String::from("example");
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();

        if let Some(title) = trimmed.strip_prefix('#') {
            heading = title.trim_start_matches('#').trim().to_owned();
            continue;
        }

        let has_comments = match trimmed {
            "```json" => false,
            "```jsonc" => true,
            _ => continue,
        };

        let lines_in_block: Vec<&str> = lines
            .by_ref()
            .take_while(|body_line| body_line.trim_start() != "```")
            .collect();
        let body = lines_in_block.join("\n");
        let body = if has_comments {
            strip_line_comments(&body)
        } else {
            body
        };
        examples.push(Example {
            heading: heading.clone(),
            body,
        });
    }

    examples
}

/// Removes `//` comments that are outside JSON strings.
fn strip_line_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());

    for line in text.lines() {
        let mut in_string = false;
        let mut escaped = false;
        let mut end = line.len();

        for (index, character) in line.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
            } else if character == '"' {
                in_string = true;
            } else if line[index..].starts_with("//") {
                end = index;
                break;
            }
        }

        output.push_str(line[..end].trim_end());
        output.push('\n');
    }

    output
}

fn slug(heading: &str) -> String {
    let mut slug = String::new();
    for character in heading.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        String::from("example")
    } else {
        slug.to_owned()
    }
}

fn unique_name(base: &str, used_names: &mut BTreeMap<String, usize>) -> String {
    let count = used_names.entry(base.to_owned()).or_insert(0);
    *count += 1;

    if *count == 1 {
        base.to_owned()
    } else {
        format!("{base}-{count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_example_under_nearest_heading() {
        let text = "## User\n###### Example User\n\n```json\n{\"id\": \"1\"}\n```\n";

        let examples = extract_examples(text);

        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].heading, "Example User");
        assert_eq!(examples[0].body, "{\"id\": \"1\"}");
    }

    #[test]
    fn strips_comments_outside_strings_only() {
        let text = "{\n  \"type\": 2,  // ComponentType.BUTTON\n  \"url\": \"https://a.b//c\"\n}";

        let stripped = strip_line_comments(text);
        let value: serde_json::Value = serde_json::from_str(&stripped).unwrap();

        assert_eq!(value["type"], 2);
        assert_eq!(value["url"], "https://a.b//c");
    }

    #[test]
    fn makes_slug_from_heading() {
        assert_eq!(slug("Example Partial Guild"), "example-partial-guild");
        assert_eq!(slug("  !!  "), "example");
    }

    #[test]
    fn makes_repeated_names_unique() {
        let mut used_names = BTreeMap::new();

        assert_eq!(unique_name("example", &mut used_names), "example");
        assert_eq!(unique_name("example", &mut used_names), "example-2");
    }
}
