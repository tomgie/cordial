//! Development tasks for the cordial workspace.
//!
//! Run a task with `cargo xtask <task>`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod fixtures;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: cargo xtask <task>

Tasks:
  fixtures <docs-dir>   Extract the JSON examples from the Discord docs into fixtures/docs.
                        <docs-dir> is the developers/ folder of a discord/discord-api-docs checkout.
  spec-check            Compare the library with the Discord OpenAPI specification (not done yet).
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
        ["spec-check"] => {
            eprintln!("spec-check is not done yet. See docs/ROADMAP.md, phase P2.");
            ExitCode::FAILURE
        }
        _ => {
            print!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
