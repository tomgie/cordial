//! A read-only check of a bot token. It sends only GET requests and changes nothing.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example live_check`.

#![expect(clippy::print_stdout, reason = "the program prints its report")]

use cordial::http::{Client, Token};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let http = Client::new(Token::from_env("DISCORD_TOKEN")?);

    let user = http.current_user().await?;
    println!("user: {} (id {}, bot {})", user.username, user.id, user.bot);

    let application = http.current_application().await?;
    println!(
        "application: {} (id {}), flags {:?}",
        application.name,
        application.id,
        application.flags()
    );

    let gateway = http.gateway_bot().await?;
    let limit = gateway.session_start_limit;
    println!(
        "gateway: recommended shards {}, session starts {}/{} left, max concurrency {}",
        gateway.shards, limit.remaining, limit.total, limit.max_concurrency
    );

    let guilds = http.current_user_guilds().await?;
    println!("guilds: {}", guilds.len());
    for guild in &guilds {
        println!("  - {} (id {})", guild.name, guild.id);
    }

    let commands = http.global_commands(application.id).await?;
    println!("global commands: {}", commands.len());
    for command in &commands {
        println!("  - {} (id {})", command.name, command.id);
    }
    Ok(())
}
