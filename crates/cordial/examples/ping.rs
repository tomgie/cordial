//! The smallest bot: a `/ping` command and a log line at READY.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example ping`.

use cordial::framework::{Bot, Command, CommandContext, Context};
use cordial::http::Token;
use cordial::model::gateway::event::Ready;

/// Checks that the bot is online.
#[derive(Command)]
#[command(name = "ping", description = "Check that the bot is online")]
struct Ping;

async fn ping(ctx: CommandContext<()>, _: Ping) -> Result<(), cordial::framework::BoxError> {
    let latency = ctx.shard(0).and_then(|shard| shard.latency()).map_or_else(
        || "unknown".to_owned(),
        |latency| format!("{} ms", latency.as_millis()),
    );
    ctx.reply(format!("Pong! Gateway latency: {latency}."))
        .await?;
    Ok(())
}

async fn ready(_: Context<()>, ready: Ready) {
    tracing::info!(user = %ready.user.username, guilds = ready.guilds.len(), "the bot is ready");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .command(ping)
        .on(ready)
        .build()?;

    bot.run().await?;
    Ok(())
}
