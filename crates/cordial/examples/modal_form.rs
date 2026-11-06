//! A feedback form in a modal.
//!
//! `/feedback` opens the modal. The modal handler gets the typed form. The custom ID of the
//! modal carries the channel for the answer.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example modal_form`.

use cordial::framework::{Bot, BoxError, Command, CommandContext, ComponentContext, Modal};
use cordial::http::Token;
use cordial::model::embed::Embed;
use cordial::model::id::ChannelId;

/// Opens the feedback form.
#[derive(Command)]
#[command(name = "feedback", description = "Send feedback to the team")]
struct OpenFeedback;

// ANCHOR: modal
/// The feedback form.
#[derive(Modal)]
#[modal(title = "Feedback")]
struct Feedback {
    #[field(label = "Subject", max_length = 80, placeholder = "A short summary")]
    subject: String,
    #[field(label = "Details", style = "paragraph", max_length = 2000)]
    details: Option<String>,
}
// ANCHOR_END: modal

// ANCHOR: handlers
async fn open_feedback(ctx: CommandContext<()>, _: OpenFeedback) -> Result<(), BoxError> {
    let channel_id = ctx.channel_id()?;
    ctx.modal(Feedback::modal(format!("feedback:{channel_id}")))
        .await?;
    Ok(())
}

async fn submit_feedback(ctx: ComponentContext<()>, form: Feedback) -> Result<(), BoxError> {
    let channel_id: ChannelId = ctx.param("channel")?;

    let embed = Embed::new()
        .title(form.subject)
        .description(form.details.unwrap_or_else(|| "(no details)".to_owned()));
    ctx.reply(embed).ephemeral().await?;
    tracing::info!(%channel_id, user = %ctx.user().username, "feedback received");
    Ok(())
}
// ANCHOR_END: handlers

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // ANCHOR: builder
    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .command(open_feedback)
        .modal("feedback:{channel}", submit_feedback)
        .build()?;
    // ANCHOR_END: builder

    bot.run().await?;
    Ok(())
}
