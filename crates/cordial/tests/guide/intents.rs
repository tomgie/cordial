use cordial::cache::ResourceTypes;
use cordial::framework::{Bot, Context, Error, IntentCheck};
use cordial::http::Token;
use cordial::model::gateway::Intents;
use cordial::model::gateway::event::{GuildMemberAdd, MessageCreate};

// ANCHOR: computed
async fn welcome(_: Context<()>, member: GuildMemberAdd) {
    let user_id = member.user.as_ref().map(|user| user.id);
    tracing::info!(?user_id, "a member joined");
}

async fn log_message(_: Context<()>, message: MessageCreate) {
    tracing::info!(author = %message.author.username, "a message arrived");
}

/// The bot computes `GUILDS`, `GUILD_MEMBERS`, `GUILD_MESSAGES`, and `DIRECT_MESSAGES`
/// from the handlers and the cache. It adds `MESSAGE_CONTENT` when the portal permits it.
fn computed_intents(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .cache(ResourceTypes::GUILD | ResourceTypes::CHANNEL)
        .on(welcome)
        .on(log_message)
        .build()
}
// ANCHOR_END: computed

// ANCHOR: explicit
fn explicit_intents(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .intents(Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT)
        .intent_check(IntentCheck::Strict)
        .on(log_message)
        .build()
}
// ANCHOR_END: explicit
