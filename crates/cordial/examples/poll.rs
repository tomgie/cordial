//! A poll bot with buttons.
//!
//! - The vote buttons use the `custom_id` pattern `vote:{poll}:{choice}`. The component router
//!   handles them, also after a restart of the bot.
//! - The "Close" button uses a collector in the command handler: a short, live conversation.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example poll`.

use std::collections::HashMap;
use std::time::Duration;

use cordial::framework::{Bot, BoxError, Command, CommandContext, ComponentContext};
use cordial::http::Token;
use cordial::model::component::{ActionRow, Button, ButtonStyle, Component};
use cordial::model::id::UserId;
use cordial::model::interaction::InteractionMessage;
use tokio::sync::Mutex;

/// The votes of each poll, by user. A real bot keeps them in a database.
#[derive(Default)]
struct State {
    polls: Mutex<HashMap<u64, Poll>>,
}

struct Poll {
    question: String,
    choices: Vec<String>,
    votes: HashMap<UserId, usize>,
    closed: bool,
}

impl Poll {
    fn message(&self, poll_id: u64) -> InteractionMessage {
        let mut counts = vec![0_usize; self.choices.len()];
        for choice in self.votes.values() {
            counts[*choice] += 1;
        }
        let lines: Vec<String> = self
            .choices
            .iter()
            .zip(&counts)
            .map(|(choice, count)| format!("**{choice}**: {count}"))
            .collect();
        let status = if self.closed { " (closed)" } else { "" };
        let content = format!("{}{status}\n{}", self.question, lines.join("\n"));

        let buttons = self.choices.iter().enumerate().map(|(index, choice)| {
            Button::new(
                ButtonStyle::Primary,
                format!("vote:{poll_id}:{index}"),
                choice,
            )
            .disabled(self.closed)
        });
        let close = Button::new(ButtonStyle::Danger, format!("close:{poll_id}"), "Close")
            .disabled(self.closed);
        InteractionMessage::text(content)
            .component(ActionRow::new(buttons.map(Component::from)))
            .component(ActionRow::new([close]))
    }
}

/// Starts a poll.
#[derive(Command)]
#[command(name = "poll", description = "Start a poll", contexts = "guild")]
struct StartPoll {
    #[option(description = "The question", max_length = 200)]
    question: String,
    #[option(
        description = "The choices, separated by commas (2 to 5)",
        max_length = 400
    )]
    choices: String,
}

async fn start_poll(ctx: CommandContext<State>, args: StartPoll) -> Result<(), BoxError> {
    let choices: Vec<String> = args
        .choices
        .split(',')
        .map(|choice| choice.trim().to_owned())
        .filter(|choice| !choice.is_empty())
        .take(5)
        .collect();
    if choices.len() < 2 {
        ctx.reply("Give at least two choices.").ephemeral().await?;
        return Ok(());
    }

    let poll_id = ctx.interaction().id.get();
    let poll = Poll {
        question: args.question,
        choices,
        votes: HashMap::new(),
        closed: false,
    };
    ctx.reply(poll.message(poll_id)).await?;
    ctx.state().polls.lock().await.insert(poll_id, poll);

    // Only the author can close the poll. The collector takes the press, so the router does
    // not see it.
    let author = ctx.user_id();
    let close_id = format!("close:{poll_id}");
    let press = ctx
        .wait_for_component(
            move |interaction| {
                interaction.author_id() == Some(author)
                    && interaction
                        .component_data()
                        .is_some_and(|data| data.custom_id == close_id)
            },
            Duration::from_secs(600),
        )
        .await;

    let mut polls = ctx.state().polls.lock().await;
    let Some(poll) = polls.get_mut(&poll_id) else {
        return Ok(());
    };
    poll.closed = true;
    let message = poll.message(poll_id);
    drop(polls);

    match press {
        Some(press) => press.update_message(message).await?,
        None => {
            ctx.edit_reply(message).await?;
        }
    }
    Ok(())
}

async fn vote(ctx: ComponentContext<State>) -> Result<(), BoxError> {
    let poll_id: u64 = ctx.param("poll")?;
    let choice: usize = ctx.param("choice")?;

    let mut polls = ctx.state().polls.lock().await;
    let Some(poll) = polls.get_mut(&poll_id).filter(|poll| !poll.closed) else {
        drop(polls);
        ctx.reply("This poll is closed.").ephemeral().await?;
        return Ok(());
    };
    poll.votes.insert(ctx.user_id(), choice);
    let message = poll.message(poll_id);
    drop(polls);

    ctx.update_message(message).await?;
    Ok(())
}

async fn close_by_other(ctx: ComponentContext<State>) -> Result<(), BoxError> {
    ctx.reply("Only the author can close the poll.")
        .ephemeral()
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .state(State::default())
        .command(start_poll)
        .component("vote:{poll}:{choice}", vote)
        .component("close:{poll}", close_by_other)
        .build()?;

    bot.run().await?;
    Ok(())
}
