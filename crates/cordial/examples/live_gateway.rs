//! A live test of the gateway and the cache. It sends no messages and changes nothing.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example live_gateway -- [seconds]`.
//!
//! The steps:
//! 1. Connect all shards. Put each event into the cache. Request the members of one guild.
//! 2. After the given time (default 45 seconds), close the shards and keep the sessions.
//! 3. Start new shards that resume the sessions. Each shard must resume, not identify.
//!
//! The program fails when an event cannot be decoded, when a shard stops with a fatal error,
//! or when a resume does not work.

#![expect(clippy::print_stdout, reason = "the program prints its report")]

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use cordial::cache::{Cache, ResourceTypes};
use cordial::gateway::{Lifecycle, Session, ShardEvent, ShardManager};
use cordial::http::{Client, Token};
use cordial::model::gateway::Intents;
use cordial::model::gateway::command::RequestGuildMembers;
use cordial::model::gateway::event::Event;
use tokio::time::{Instant, timeout_at};

/// The intents of the test. `GUILD_MEMBERS` is privileged: turn it on in the Developer Portal.
fn intents() -> Intents {
    Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MODERATION
        | Intents::GUILD_EXPRESSIONS
        | Intents::GUILD_VOICE_STATES
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MESSAGE_REACTIONS
        | Intents::GUILD_SCHEDULED_EVENTS
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let seconds = std::env::args()
        .nth(1)
        .map_or(Ok(45), |text| text.parse::<u64>())?;
    let http = Client::new(Token::from_env("DISCORD_TOKEN")?);
    let cache = Cache::new(ResourceTypes::all());

    let mut manager = ShardManager::builder(http.clone(), intents())
        .start()
        .await?;
    println!("started {} shard(s)", manager.shard_count());

    let mut report = Report::default();
    let mut requested_members = false;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while let Ok(Some(message)) = timeout_at(deadline, manager.next_event()).await {
        if let ShardEvent::Dispatch(event) = &message.event {
            cache.update(event);
            if !requested_members && let Event::GuildCreate(_) = event.as_ref() {
                requested_members = true;
                let guild_id = cache.guild_ids().into_iter().next();
                if let (Some(guild_id), Some(shard)) = (guild_id, manager.shards().first()) {
                    let mut chunks = shard
                        .request_members(RequestGuildMembers::all(guild_id))
                        .await?;
                    tokio::spawn(async move {
                        let mut members = 0;
                        while let Some(chunk) = chunks.next().await {
                            members += chunk.members.len();
                        }
                        println!("member request: {members} members in guild {guild_id}");
                    });
                }
            }
        }
        report.add(&message.event)?;
    }

    for shard in manager.shards() {
        println!("shard {} latency: {:?}", shard.id(), shard.latency());
    }
    println!("cache: {:?}", cache.stats());
    report.print();

    let sessions = manager.close_resumable().await;
    println!("closed with {} resumable session(s)", sessions.len());
    if sessions.is_empty() {
        return Err("no session to resume".into());
    }

    resume(http, sessions).await
}

/// Starts new shards with the sessions and checks that each one resumes.
async fn resume(
    http: Client,
    sessions: HashMap<u32, Session>,
) -> Result<(), Box<dyn std::error::Error>> {
    let count = sessions.len();
    let mut manager = ShardManager::builder(http, intents())
        .resume_sessions(sessions)
        .start()
        .await?;

    let mut resumed = 0;
    let mut replayed = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    while resumed < count {
        let Ok(Some(message)) = timeout_at(deadline, manager.next_event()).await else {
            break;
        };
        match message.event {
            ShardEvent::Lifecycle(Lifecycle::Resumed) => resumed += 1,
            ShardEvent::Lifecycle(Lifecycle::Ready { .. }) => {
                return Err("a shard identified again instead of a resume".into());
            }
            ShardEvent::Fatal(error) => return Err(format!("fatal: {error}").into()),
            ShardEvent::Dispatch(_) => replayed += 1,
            _ => {}
        }
    }
    manager.close().await;

    println!("resume: {resumed}/{count} shard(s) resumed, {replayed} event(s) replayed");
    if resumed < count {
        return Err("not all shards resumed within 30 seconds".into());
    }
    Ok(())
}

/// The counts of the events and the problems.
#[derive(Default)]
struct Report {
    events: BTreeMap<String, usize>,
    lifecycle: Vec<String>,
}

impl Report {
    fn add(&mut self, event: &ShardEvent) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            ShardEvent::Dispatch(event) => {
                *self.events.entry(event.name().to_owned()).or_default() += 1;
            }
            ShardEvent::Lifecycle(lifecycle) => self.lifecycle.push(format!("{lifecycle:?}")),
            ShardEvent::DecodeError { name, error } => {
                return Err(format!("cannot decode {name}: {error}").into());
            }
            ShardEvent::Fatal(error) => return Err(format!("fatal: {error}").into()),
            other => self.lifecycle.push(format!("{other:?}")),
        }
        Ok(())
    }

    fn print(&self) {
        println!("lifecycle: {:?}", self.lifecycle);
        for (name, count) in &self.events {
            println!("  {name}: {count}");
        }
    }
}
