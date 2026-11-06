use std::time::Duration;

use cordial::framework::{Bot, BoxError, Command, CommandContext, Error, SlowResponse};
use cordial::http::Token;
use cordial::model::Color;
use cordial::model::component::{Container, Separator, TextDisplay};
use cordial::model::embed::Embed;
use cordial::model::interaction::InteractionMessage;

// ANCHOR: defer
/// Makes a report. The work takes longer than 3 seconds.
#[derive(Command)]
#[command(name = "report", description = "Make a report")]
struct Report;

async fn report(ctx: CommandContext<()>, _: Report) -> Result<(), BoxError> {
    // The user sees "thinking" until the next reply.
    ctx.defer_ephemeral().await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    // After a defer, `reply` edits the original response. The visibility must match the
    // defer: else the bot deletes the loading message and sends a follow-up message.
    let embed = Embed::new()
        .title("Report")
        .description("All systems work.");
    ctx.reply(embed).ephemeral().await?;

    // Now the interaction has a response, so `reply` sends a follow-up message.
    ctx.reply("A second message.").ephemeral().await?;

    // `edit_reply` changes the first message again.
    ctx.edit_reply(InteractionMessage::text("The report is old."))
        .await?;
    Ok(())
}
// ANCHOR_END: defer

// ANCHOR: slow
fn with_auto_defer(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        // Defer each interaction that has no response after 2 seconds.
        .slow_response(SlowResponse::Defer(Duration::from_secs(2)))
        .command(report)
        .build()
}
// ANCHOR_END: slow

// ANCHOR: v2
/// Shows the status in a Components V2 message.
#[derive(Command)]
#[command(name = "status", description = "Show the status")]
struct Status;

async fn status(ctx: CommandContext<()>, _: Status) -> Result<(), BoxError> {
    let mut panel = Container::new(vec![
        TextDisplay::new("## Status").into(),
        Separator::default().into(),
        TextDisplay::new("All systems work.").into(),
    ]);
    panel.accent_color = Some(Color::from_rgb(0x2E, 0xCC, 0x71));

    let message = InteractionMessage::new().component(panel).components_v2();
    ctx.reply(message).ephemeral().await?;
    Ok(())
}
// ANCHOR_END: v2
