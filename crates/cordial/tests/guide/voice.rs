use std::time::Duration;

use cordial::cache::ResourceTypes;
use cordial::framework::{Bot, BoxError, Command, CommandContext, Error};
use cordial::http::Token;
use cordial::voice::{OggOpusSource, Track, TrackEvent, VoiceEvent};

// ANCHOR: play
/// Plays the intro in the voice channel of the user.
#[derive(Command)]
#[command(name = "intro", description = "Play the intro", contexts = "guild")]
struct Intro;

async fn intro(ctx: CommandContext<()>, _: Intro) -> Result<(), BoxError> {
    let guild_id = ctx.guild_id()?;
    let cache = ctx.cache().ok_or("the bot has no cache")?;
    let channel_id = cache
        .voice_state(guild_id, ctx.user_id())
        .and_then(|state| state.channel_id)
        .ok_or("join a voice channel first")?;
    let manager = ctx.voice().ok_or("voice is not ready")?;

    // A join can take more than 3 seconds.
    ctx.defer().await?;
    let call = manager.join(guild_id, channel_id).await?;
    let track = call.enqueue(Track::new(OggOpusSource::open("intro.ogg")?).volume(0.8));
    ctx.reply("The intro plays.").await?;

    // Skip the first 5 seconds, then wait for the end of the track.
    track.seek(Duration::from_secs(5));
    let mut events = call.subscribe();
    while let Ok(event) = events.recv().await {
        if let VoiceEvent::Track(TrackEvent::Ended { track: id, reason }) = event
            && id == track.id()
        {
            tracing::info!(?reason, "the intro ended");
            break;
        }
    }
    manager.leave(guild_id).await?;
    Ok(())
}
// ANCHOR_END: play

// ANCHOR: bot
fn voice_bot(token: Token) -> Result<Bot<()>, Error> {
    Bot::builder(token)
        // The cache gives the voice channel of the user.
        .cache(ResourceTypes::GUILD | ResourceTypes::VOICE_STATE)
        // The bot makes the voice manager with DAVE and gives it the voice events.
        .voice()
        .command(intro)
        .build()
}
// ANCHOR_END: bot

#[test]
fn voice_bot_builds() {
    assert!(voice_bot(Token::bot("abc")).is_ok());
}
