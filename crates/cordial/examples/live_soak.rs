//! A long live test of the gateway and the cache. It sends no messages and changes nothing.
//!
//! Record: `DISCORD_TOKEN=... cargo run --example live_soak -- run <hours> <shards>
//! <restart minutes> <log file>`.
//! Replay: `cargo run --example live_soak -- replay <log file>`.
//!
//! The `run` mode:
//! - Runs the shards for the given time and puts each event into the cache.
//! - Writes each event to the log file as one JSON line.
//! - Checks each event: no sequence gap, no decode error, and `decode(encode(event))` gives
//!   the same event.
//! - At each restart interval, closes the shards and starts new shards that resume the
//!   sessions. This is a forced disconnect.
//! - Writes a status line each 10 minutes.
//!
//! The `replay` mode decodes each event of a log and puts it into a new cache.
//!
//! The program fails when a check fails or a shard stops with a fatal error.

#![expect(clippy::print_stdout, reason = "the program prints its report")]

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::time::Duration;

use cordial::cache::{Cache, ResourceTypes};
use cordial::gateway::{Lifecycle, Session, ShardEvent, ShardManager, ShardMessage, ShardRange};
use cordial::http::{Client, Token};
use cordial::model::gateway::Intents;
use cordial::model::gateway::event::Event;
use serde_json::value::RawValue;
use tokio::time::{Instant, timeout_at};

type BoxError = Box<dyn std::error::Error>;

const STATUS_INTERVAL: Duration = Duration::from_secs(600);

/// The intents of the test. `GUILD_MEMBERS` is privileged: turn it on in the Developer Portal.
fn intents() -> Intents {
    Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MODERATION
        | Intents::GUILD_EXPRESSIONS
        | Intents::GUILD_INTEGRATIONS
        | Intents::GUILD_WEBHOOKS
        | Intents::GUILD_INVITES
        | Intents::GUILD_VOICE_STATES
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MESSAGE_REACTIONS
        | Intents::GUILD_MESSAGE_TYPING
        | Intents::DIRECT_MESSAGES
        | Intents::DIRECT_MESSAGE_REACTIONS
        | Intents::GUILD_SCHEDULED_EVENTS
        | Intents::AUTO_MODERATION_CONFIGURATION
        | Intents::AUTO_MODERATION_EXECUTION
        | Intents::GUILD_MESSAGE_POLLS
        | Intents::DIRECT_MESSAGE_POLLS
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["run", hours, shards, restart_minutes, log] => {
            let settings = RunSettings {
                duration: Duration::from_secs_f64(hours.parse::<f64>()? * 3600.0),
                shards: shards.parse()?,
                restart_interval: Duration::from_secs(restart_minutes.parse::<u64>()? * 60),
            };
            run(settings, File::create(log)?).await
        }
        ["replay", log] => replay(File::open(log)?),
        _ => Err("usage: run <hours> <shards> <restart minutes> <log> | replay <log>".into()),
    }
}

struct RunSettings {
    duration: Duration,
    shards: u32,
    restart_interval: Duration,
}

async fn run(settings: RunSettings, log: File) -> Result<(), BoxError> {
    let http = Client::new(Token::from_env("DISCORD_TOKEN")?);
    let cache = Cache::new(ResourceTypes::all());
    let mut log = BufWriter::new(log);
    let mut stats = Stats::default();

    let end = Instant::now() + settings.duration;
    let mut status_at = Instant::now() + STATUS_INTERVAL;
    let mut sessions: HashMap<u32, Session> = HashMap::new();
    loop {
        let restart_at = (Instant::now() + settings.restart_interval).min(end);
        stats.expected_resumes += sessions.len();
        let mut manager = ShardManager::builder(http.clone(), intents())
            .shards(ShardRange::All {
                count: settings.shards,
            })
            .resume_sessions(std::mem::take(&mut sessions))
            .start()
            .await?;

        loop {
            let wake = restart_at.min(status_at);
            match timeout_at(wake, manager.next_event()).await {
                Ok(Some(message)) => stats.handle(&message, &cache, &mut log)?,
                Ok(None) => return Err("all shards stopped".into()),
                Err(_) => {}
            }

            let now = Instant::now();
            if now >= status_at {
                log.flush()?;
                stats.print_status(&cache, &manager);
                status_at = now + STATUS_INTERVAL;
            }
            if now >= restart_at {
                break;
            }
        }

        if Instant::now() >= end {
            manager.close().await;
            break;
        }
        sessions = manager.close_resumable().await;
        stats.restarts += 1;
    }

    log.flush()?;
    stats.print_summary(&cache);
    stats.result()
}

/// The counts of the run.
#[derive(Default)]
struct Stats {
    events: BTreeMap<String, u64>,
    lifecycle: BTreeMap<String, u64>,
    disconnects: BTreeMap<String, u64>,
    gaps: Vec<String>,
    decode_errors: Vec<String>,
    round_trip_errors: Vec<String>,
    restarts: u64,
    expected_resumes: usize,
    resumes: usize,
}

impl Stats {
    fn handle(
        &mut self,
        message: &ShardMessage,
        cache: &Cache,
        log: &mut impl Write,
    ) -> Result<(), BoxError> {
        match &message.event {
            ShardEvent::Dispatch(event) => {
                *self.events.entry(event.name().to_owned()).or_default() += 1;
                cache.update(event);
                let data = event.encode()?;
                self.check_round_trip(event, &data);
                writeln!(
                    log,
                    r#"{{"shard":{},"name":{},"data":{}}}"#,
                    message.shard_id,
                    serde_json::to_string(event.name())?,
                    data.get()
                )?;
            }
            ShardEvent::Lifecycle(lifecycle) => self.add_lifecycle(message.shard_id, lifecycle),
            ShardEvent::Disconnected { code } => {
                *self.disconnects.entry(format!("{code:?}")).or_default() += 1;
            }
            ShardEvent::DecodeError { name, error } => {
                println!("decode error: {name}: {error}");
                self.decode_errors.push(format!("{name}: {error}"));
            }
            ShardEvent::Fatal(error) => {
                return Err(format!("shard {} stopped: {error}", message.shard_id).into());
            }
            _ => {}
        }
        Ok(())
    }

    fn add_lifecycle(&mut self, shard_id: u32, lifecycle: &Lifecycle) {
        let name = match lifecycle {
            Lifecycle::Hello { .. } => "Hello",
            Lifecycle::Ready { .. } => "Ready",
            Lifecycle::Resumed => {
                self.resumes += 1;
                "Resumed"
            }
            Lifecycle::InvalidSession { resumable: true } => "InvalidSession(resumable)",
            Lifecycle::InvalidSession { resumable: false } => "InvalidSession(new session)",
            Lifecycle::ReconnectRequested => "ReconnectRequested",
            Lifecycle::Zombie => "Zombie",
            Lifecycle::HeartbeatAck { .. } => "HeartbeatAck",
            Lifecycle::SequenceGap { expected, received } => {
                println!("sequence gap on shard {shard_id}: expected {expected}, got {received}");
                self.gaps
                    .push(format!("shard {shard_id}: {expected} -> {received}"));
                "SequenceGap"
            }
            _ => "other",
        };
        *self.lifecycle.entry(name.to_owned()).or_default() += 1;
    }

    fn check_round_trip(&mut self, event: &Event, data: &RawValue) {
        let same = Event::decode(event.name(), data).is_ok_and(|decoded| decoded == *event);
        if !same {
            println!("round trip changed {}: {}", event.name(), data.get());
            self.round_trip_errors.push(event.name().to_owned());
        }
    }

    fn print_status(&self, cache: &Cache, manager: &ShardManager) {
        let latencies: Vec<_> = manager
            .shards()
            .iter()
            .map(|shard| shard.latency().map(|latency| latency.as_millis()))
            .collect();
        println!(
            "status: events {}, restarts {}, resumes {}/{}, gaps {}, decode errors {}, \
             round trip errors {}, latency ms {latencies:?}, cache {:?}",
            self.events.values().sum::<u64>(),
            self.restarts,
            self.resumes,
            self.expected_resumes,
            self.gaps.len(),
            self.decode_errors.len(),
            self.round_trip_errors.len(),
            cache.stats()
        );
    }

    fn print_summary(&self, cache: &Cache) {
        println!("summary:");
        println!("  events: {:?}", self.events);
        println!("  lifecycle: {:?}", self.lifecycle);
        println!("  disconnects: {:?}", self.disconnects);
        println!(
            "  restarts {}, resumes {}/{}",
            self.restarts, self.resumes, self.expected_resumes
        );
        println!("  gaps: {:?}", self.gaps);
        println!("  decode errors: {:?}", self.decode_errors);
        println!("  round trip errors: {:?}", self.round_trip_errors);
        println!("  cache: {:?}", cache.stats());
    }

    fn result(&self) -> Result<(), BoxError> {
        let problems = self.gaps.len() + self.decode_errors.len() + self.round_trip_errors.len();
        if problems > 0 {
            return Err(format!("{problems} problem(s); see the summary").into());
        }
        if self.resumes < self.expected_resumes {
            return Err("not all forced disconnects resumed".into());
        }
        Ok(())
    }
}

/// Decodes each event of a log and puts it into a new cache.
fn replay(log: File) -> Result<(), BoxError> {
    let cache = Cache::new(ResourceTypes::all());
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut errors = 0_u64;

    for line in BufReader::new(log).lines() {
        let line = line?;
        let record: serde_json::Value = serde_json::from_str(&line)?;
        let name = record["name"].as_str().ok_or("a record has no name")?;
        let data = serde_json::value::to_raw_value(&record["data"])?;

        match Event::decode(name, &data) {
            Ok(event) => {
                cache.update(&event);
                *counts.entry(name.to_owned()).or_default() += 1;
            }
            Err(error) => {
                println!("cannot decode {name}: {error}");
                errors += 1;
            }
        }
    }

    println!("replayed: {counts:?}");
    println!("cache: {:?}", cache.stats());
    if errors > 0 {
        return Err(format!("{errors} event(s) could not be decoded").into());
    }
    Ok(())
}
