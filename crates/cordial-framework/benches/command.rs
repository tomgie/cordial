//! The cost of command option parsing and of `custom_id` pattern matching.

use std::hint::black_box;

use cordial_framework::command::ResolvedUser;
use cordial_framework::{Command, Pattern};
use cordial_model::interaction::CommandData;
use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;

/// A typical moderation command with a resolved user and primitive options.
#[derive(Command, Debug)]
#[command(name = "moderate", description = "Moderate a member")]
#[allow(dead_code, reason = "the benchmark only parses the fields")]
struct Moderate {
    #[option(description = "The member")]
    user: ResolvedUser,
    #[option(description = "The reason", max_length = 512)]
    reason: String,
    #[option(description = "The number of days", min_value = 0, max_value = 7)]
    days: i64,
    #[option(description = "Tells if the action is silent")]
    silent: Option<bool>,
}

fn command_data() -> CommandData {
    serde_json::from_value(json!({
        "id": "866818195033292850",
        "name": "moderate",
        "type": 1,
        "options": [
            {"name": "user", "type": 6, "value": "1000002"},
            {"name": "reason", "type": 3, "value": "Spam in the general channel"},
            {"name": "days", "type": 4, "value": 7},
            {"name": "silent", "type": 5, "value": true}
        ],
        "resolved": {
            "users": {"1000002": {
                "id": "1000002", "username": "user", "discriminator": "0", "avatar": null
            }},
            "members": {"1000002": {
                "roles": [], "joined_at": "2026-01-01T00:00:00.000000+00:00", "flags": 0
            }}
        }
    }))
    .expect("valid command data")
}

fn parse_options(criterion: &mut Criterion) {
    let data = command_data();
    Moderate::parse(&data).expect("the command data matches the definition");

    let mut group = criterion.benchmark_group("command");
    group.bench_function("parse_options", |bencher| {
        bencher.iter(|| Moderate::parse(black_box(&data)));
    });

    let pattern = Pattern::parse("poll:{poll_id}:vote:{choice}").expect("a valid pattern");
    group.bench_function("pattern_match", |bencher| {
        bencher.iter(|| pattern.matches(black_box("poll:1300000000000000000:vote:yes")));
    });
    group.bench_function("pattern_no_match", |bencher| {
        bencher.iter(|| pattern.matches(black_box("ticket:1300000000000000000:close")));
    });
    group.finish();
}

criterion_group!(benches, parse_options);
criterion_main!(benches);
