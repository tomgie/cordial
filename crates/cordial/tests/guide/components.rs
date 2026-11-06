use std::time::Duration;

use cordial::framework::{
    Bot, BoxError, Command, CommandContext, ComponentContext, Error, Pattern,
};
use cordial::http::Token;
use cordial::model::component::{ActionRow, Button, ButtonStyle};
use cordial::model::gateway::event::{EventType, MessageCreate};
use cordial::model::interaction::InteractionMessage;

// ANCHOR: format
/// Makes the `custom_id` of a page button from the same pattern that the router uses.
fn page_button(page: u32) -> Button {
    let pattern = Pattern::parse("page:{number}").expect("the pattern is valid");
    let custom_id = pattern
        .format(&[("number", &page)])
        .expect("the pattern has one parameter");
    Button::new(ButtonStyle::Secondary, custom_id, format!("Page {page}"))
}

async fn show_page(ctx: ComponentContext<()>) -> Result<(), BoxError> {
    let page: u32 = ctx.param("number")?;
    let message = InteractionMessage::text(format!("This is page {page}."))
        .component(ActionRow::new([page_button(page + 1)]));
    ctx.update_message(message).await?;
    Ok(())
}
// ANCHOR_END: format

// ANCHOR: wait_for
/// Asks a question and waits for the answer in the channel.
#[derive(Command)]
#[command(name = "quiz", description = "Answer a question")]
struct Quiz;

async fn quiz(ctx: CommandContext<()>, _: Quiz) -> Result<(), BoxError> {
    ctx.reply("What is 6 times 7? Answer in 30 seconds.")
        .await?;

    let user_id = ctx.user_id();
    let channel_id = ctx.channel_id()?;
    let answer = ctx
        .wait_for(
            move |message: &MessageCreate| {
                message.author.id == user_id && message.channel_id == channel_id
            },
            Duration::from_secs(30),
        )
        .await;

    let text = match answer {
        Some(message) if message.content.trim() == "42" => "Correct!",
        Some(_) => "That is not correct.",
        None => "The time is over.",
    };
    ctx.reply(text).await?;
    Ok(())
}

fn with_collector(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .command(quiz)
        // No handler uses `MessageCreate`, so request it for the collector.
        .extra_events([EventType::MessageCreate])
        .component("page:{number}", show_page)
        .build()
}
// ANCHOR_END: wait_for
