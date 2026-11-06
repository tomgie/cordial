use cordial::cache::ResourceTypes;
use cordial::framework::{Bot, BoxError, Command, CommandContext, Error};
use cordial::http::Token;
use cordial::model::Permissions;

// ANCHOR: setup
fn with_cache(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        .cache(
            ResourceTypes::GUILD
                | ResourceTypes::CHANNEL
                | ResourceTypes::ROLE
                | ResourceTypes::MEMBER
                | ResourceTypes::USER
                | ResourceTypes::MESSAGE,
        )
        // Keep the last 50 messages of each channel.
        .cache_messages(50)
        .command(can_post)
        .build()
}
// ANCHOR_END: setup

// ANCHOR: permissions
/// Tells if the bot can post in this channel.
#[derive(Command)]
#[command(
    name = "can-post",
    description = "Check the permissions of the bot here"
)]
struct CanPost;

async fn can_post(ctx: CommandContext<()>, _: CanPost) -> Result<(), BoxError> {
    let cache = ctx.cache().ok_or("the bot has no cache")?;
    let bot_user = cache.current_user().ok_or("READY did not arrive yet")?;

    let permissions = cache.channel_permissions(ctx.channel_id()?, bot_user.id)?;
    let needed = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES;
    let text = if permissions.contains(needed) {
        "I can post here."
    } else {
        "I cannot post here."
    };

    ctx.reply(text).ephemeral().await?;
    Ok(())
}
// ANCHOR_END: permissions
