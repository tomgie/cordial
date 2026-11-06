use std::collections::HashMap;

use cordial::framework::{Bot, BoxError, ShutdownMode};
use cordial::gateway::Session;
use cordial::http::Token;
use cordial::model::gateway::UpdatePresence;
use cordial::model::gateway::presence::{Activity, Status};
use tracing_subscriber::EnvFilter;

// ANCHOR: resume
/// The file that keeps the sessions between two runs of the process.
const SESSIONS_FILE: &str = "sessions.json";

async fn run() -> Result<(), BoxError> {
    // `RUST_LOG=info,cordial_http=debug` sets the log levels.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // A missing or bad file gives no sessions: the shards identify again.
    let sessions: HashMap<u32, Session> = std::fs::read_to_string(SESSIONS_FILE)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let _ = std::fs::remove_file(SESSIONS_FILE);

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .presence(UpdatePresence::new(
            Status::Online,
            vec![Activity::playing("with commands")],
        ))
        .shutdown_mode(ShutdownMode::Resumable)
        .resume_sessions(sessions)
        .build()?;

    let outcome = bot.run().await?;
    std::fs::write(SESSIONS_FILE, serde_json::to_string(&outcome.sessions)?)?;
    Ok(())
}
// ANCHOR_END: resume
