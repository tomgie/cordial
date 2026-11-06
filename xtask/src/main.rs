//! Development tasks for the cordial workspace.
//!
//! Run a task with `cargo xtask <task>`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod fixtures;
mod spec_check;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: cargo xtask <task>

Tasks:
  fixtures <docs-dir>   Extract the JSON examples from the Discord docs into fixtures/docs.
                        <docs-dir> is the developers/ folder of a discord/discord-api-docs checkout.
  spec-check <openapi.json> [--strict]
                        Compare the endpoints of cordial-http with the Discord OpenAPI
                        specification (specs/openapi.json of discord/discord-api-spec).
                        With --strict, fail when an operation is missing.
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();

    match arguments.as_slice() {
        ["fixtures", docs_dir] => match fixtures::run(&PathBuf::from(docs_dir)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("fixtures: {error}");
                ExitCode::FAILURE
            }
        },
        ["spec-check", spec_path, options @ ..] => {
            let strict = options.contains(&"--strict");
            match spec_check::run(&PathBuf::from(spec_path)) {
                Ok(missing) if strict && missing > 0 => ExitCode::FAILURE,
                Ok(_) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("spec-check: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            print!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
