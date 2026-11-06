//! A bot that gets interactions over HTTP, with no gateway connection.
//!
//! 1. Run it: `DISCORD_TOKEN=... DISCORD_PUBLIC_KEY=... cargo run --example http_interactions
//!    --features axum`.
//! 2. Make port 8080 public with HTTPS (for example with a tunnel).
//! 3. Set the "Interactions Endpoint URL" in the Developer Portal to `https://<host>/`.
//!    Discord sends a PING to check the URL. The endpoint answers it.

use cordial::endpoint::{Endpoint, PublicKey};
use cordial::framework::{Bot, BoxError, Command, CommandContext};
use cordial::http::Token;

/// Says hello.
#[derive(Command)]
#[command(name = "hello", description = "Say hello")]
struct Hello;

async fn hello(ctx: CommandContext<()>, _: Hello) -> Result<(), BoxError> {
    ctx.reply(format!("Hello, {}!", ctx.user().username))
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .command(hello)
        .build()?;
    // Gets the application ID and syncs the commands. `run` is not called: no gateway.
    bot.prepare().await?;

    let key = PublicKey::from_hex(&std::env::var("DISCORD_PUBLIC_KEY")?)?;
    let endpoint = Endpoint::new(key, bot.endpoint_handler());
    let router = cordial::endpoint::axum::router(endpoint);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("listening on port 8080");
    axum::serve(listener, router).await?;
    Ok(())
}
