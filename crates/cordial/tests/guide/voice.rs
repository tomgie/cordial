use std::sync::OnceLock;
use std::time::Duration;

use cordial::cache::ResourceTypes;
use cordial::framework::{Bot, BoxError, Command, CommandContext, Context, Error};
use cordial::gateway::ShardHandle;
use cordial::http::Token;
use cordial::model::gateway::Event;
use cordial::model::gateway::event::{Ready, VoiceServerUpdate, VoiceStateUpdate};
use cordial::voice::{
    CallConfig, DaveSession, E2eeSession, OggOpusSource, Track, TrackEvent, VoiceEvent,
    VoiceManager,
};

// ANCHOR: manager
/// The voice manager needs the shard handles. They exist after the bot starts.
#[derive(Default)]
struct State {
    voice: OnceLock<VoiceManager>,
}

/// Makes a manager that uses DAVE end-to-end encryption in each call.
fn voice_manager(shards: Vec<ShardHandle>) -> VoiceManager {
    VoiceManager::with_e2ee(shards, CallConfig::default(), || {
        let session = DaveSession::new().expect("the DAVE key pair is made");
        Box::new(session) as Box<dyn E2eeSession>
    })
}

async fn ready(ctx: Context<State>, ready: Ready) {
    let manager = ctx
        .state()
        .voice
        .get_or_init(|| voice_manager(ctx.shards()));
    manager.set_user_id(ready.user.id);
}

async fn voice_state(ctx: Context<State>, update: VoiceStateUpdate) {
    if let Some(manager) = ctx.state().voice.get() {
        manager.handle_event(&Event::VoiceStateUpdate(Box::new(update)));
    }
}

async fn voice_server(ctx: Context<State>, update: VoiceServerUpdate) {
    if let Some(manager) = ctx.state().voice.get() {
        manager.handle_event(&Event::VoiceServerUpdate(Box::new(update)));
    }
}
// ANCHOR_END: manager

// ANCHOR: play
/// Plays the intro in the voice channel of the user.
#[derive(Command)]
#[command(name = "intro", description = "Play the intro", contexts = "guild")]
struct Intro;

async fn intro(ctx: CommandContext<State>, _: Intro) -> Result<(), BoxError> {
    let guild_id = ctx.guild_id()?;
    let cache = ctx.cache().ok_or("the bot has no cache")?;
    let channel_id = cache
        .voice_state(guild_id, ctx.user_id())
        .and_then(|state| state.channel_id)
        .ok_or("join a voice channel first")?;
    let manager = ctx.state().voice.get().ok_or("voice is not ready")?;

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

fn voice_bot(token: Token) -> Result<Bot<State>, Error> {
    Bot::builder(token)
        .state(State::default())
        .cache(ResourceTypes::GUILD | ResourceTypes::VOICE_STATE)
        .on(ready)
        .on(voice_state)
        .on(voice_server)
        .command(intro)
        .build()
}
// ANCHOR_END: play
