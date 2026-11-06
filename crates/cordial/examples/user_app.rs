//! A user-installable app: commands that work in all servers and DMs of the user.
//!
//! - `/roll` works where the user installed the app.
//! - "Quote" is a message command: right-click a message, then Apps, then Quote.
//! - "Avatar" is a user command.
//!
//! The app needs "User Install" in the Installation page of the Developer Portal. A user app
//! gets interactions only, so the bot needs no intents. This example still uses the gateway;
//! the `endpoint` feature gives interactions over HTTP in place of it.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example user_app`.

use cordial::framework::{Bot, BoxError, Command, CommandContext, command::ResolvedUser};
use cordial::http::Token;
use cordial::model::embed::Embed;
use cordial::model::gateway::Intents;
use cordial::model::message::Message;

// ANCHOR: roll
/// Rolls a die.
#[derive(Command)]
#[command(
    name = "roll",
    description = "Roll a die",
    integration_types = "guild_install, user_install",
    contexts = "guild, bot_dm, private_channel"
)]
struct Roll {
    #[option(description = "The number of sides", min_value = 2, max_value = 1000)]
    sides: Option<u16>,
}
// ANCHOR_END: roll

async fn roll(ctx: CommandContext<()>, args: Roll) -> Result<(), BoxError> {
    let sides = u64::from(args.sides.unwrap_or(6));
    // The interaction ID is a snowflake with a millisecond time: random enough for a game.
    let result = ctx.interaction().id.get() % sides + 1;
    ctx.reply(format!("🎲 {result} (1 to {sides})")).await?;
    Ok(())
}

// ANCHOR: targets
/// Quotes a message.
#[derive(Command)]
#[command(
    name = "Quote",
    kind = "message",
    integration_types = "guild_install, user_install",
    contexts = "guild, bot_dm, private_channel"
)]
struct Quote(Message);

async fn quote(ctx: CommandContext<()>, Quote(message): Quote) -> Result<(), BoxError> {
    let embed = Embed::new()
        .description(message.content.clone())
        .title(format!("{} said", message.author.username))
        .url(message.link());
    ctx.reply(embed).await?;
    Ok(())
}

/// Shows the avatar of a user.
#[derive(Command)]
#[command(
    name = "Avatar",
    kind = "user",
    integration_types = "guild_install, user_install",
    contexts = "guild, bot_dm, private_channel"
)]
struct Avatar(ResolvedUser);

async fn avatar(ctx: CommandContext<()>, Avatar(target): Avatar) -> Result<(), BoxError> {
    let url = target.user.avatar_url().to_string();
    ctx.reply(url).ephemeral().await?;
    Ok(())
}
// ANCHOR_END: targets

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .intents(Intents::empty())
        .command(roll)
        .command(quote)
        .command(avatar)
        .build()?;

    bot.run().await?;
    Ok(())
}
