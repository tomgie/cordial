//! A live test of a voice call with DAVE end-to-end encryption. It sends no messages and
//! changes no commands.
//!
//! Run it with `DISCORD_TOKEN=... cargo run --example live_voice --features opus --
//! <guild id> [seconds] [channel id]`.
//!
//! The steps:
//! 1. Connect to the gateway. Wait until a person joins a voice channel and is alone in it.
//!    People who are in a voice channel at the start do not count. Thus the bot does not
//!    join other people. With a channel ID, the bot joins only that channel, and also when one
//!    person is in it at the start.
//! 2. Join that channel with a DAVE session. The call must use DAVE protocol version 1.
//! 3. Play a test tone: three rising notes, then a pause, again and again.
//! 4. Receive the audio of each person. Write a status line each 5 seconds with the number of
//!    frames, the peak level, and the short-term loudness (EBU R128, 3 s window).
//! 5. After the given time (default 120 seconds), leave the channel and write a summary with
//!    the integrated loudness and the true peak of each person, and of the test tone.
//!
//! The program fails when the call does not use DAVE, or when a DAVE error occurs.

#![expect(clippy::print_stdout, reason = "the program prints its report")]

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use cordial::gateway::{ShardEvent, ShardManager};
use cordial::http::{Client, Token};
use cordial::model::gateway::Intents;
use cordial::model::gateway::event::{Event, GuildCreate};
use cordial::model::id::{ChannelId, GuildId, UserId};
use ebur128::{EbuR128, Mode};

use cordial::voice::{
    CallConfig, DaveSession, E2eeSession, PcmSource, ShardHandles, Track, VoiceEvent, VoiceManager,
};
use tokio::time::{Instant, sleep_until, timeout_at};

type BoxError = Box<dyn std::error::Error>;

const SAMPLE_RATE: f32 = 48_000.0;
const STATUS_INTERVAL: Duration = Duration::from_secs(5);
const WAIT_FOR_PERSON: Duration = Duration::from_secs(300);

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // RUST_LOG can turn on the debug and trace logs of the voice receive path.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let guild_id: GuildId = args
        .first()
        .ok_or("usage: live_voice <guild id> [seconds] [channel id]")?
        .parse()?;
    let seconds = args.get(1).map_or(Ok(120), |text| text.parse::<u64>())?;
    let only_channel: Option<ChannelId> = args.get(2).map(|text| text.parse()).transpose()?;

    let http = Client::new(Token::from_env("DISCORD_TOKEN")?);
    let mut gateway = ShardManager::builder(http, Intents::GUILDS | Intents::GUILD_VOICE_STATES)
        .start()
        .await?;
    let config = CallConfig::default().receive(true).decode(true);
    let voice = VoiceManager::with_e2ee(
        ShardHandles::new(gateway.shards().to_vec(), gateway.shard_count()),
        config,
        || Box::new(DaveSession::new().expect("a DAVE key pair")) as Box<dyn E2eeSession>,
    );

    let (channel_id, person) =
        wait_for_person(&mut gateway, &voice, guild_id, only_channel).await?;
    println!("person {person} is in channel {channel_id}; joining");

    // The gateway events must reach the voice manager during the call too.
    let events_voice = voice.clone();
    let event_task = tokio::spawn(async move {
        while let Some(message) = gateway.next_event().await {
            if let ShardEvent::Dispatch(event) = &message.event {
                events_voice.handle_event(event);
            }
        }
    });

    // Subscribe while the join runs. The Ready event, with the DAVE version, comes before the
    // join returns.
    let join_voice = voice.clone();
    let join = tokio::spawn(async move { join_voice.join(guild_id, channel_id).await });
    let mut events = loop {
        if let Some(call) = voice.call(guild_id) {
            break call.subscribe();
        }
        if join.is_finished() {
            return Err(format!("the join failed: {:?}", join.await?).into());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    let call = join.await??;
    let tone = test_tone(seconds);
    println!("test tone: {}", Loudness::of_samples(&tone).describe());
    call.play(Track::new(PcmSource::from_f32(tone)));

    let mut report = Report::default();
    let end = Instant::now() + Duration::from_secs(seconds);
    let mut status_at = Instant::now() + STATUS_INTERVAL;
    loop {
        let wake = end.min(status_at);
        match timeout_at(wake, events.recv()).await {
            Ok(Ok(event)) => report.add(event),
            Ok(Err(error)) => println!("event receiver: {error}"),
            Err(_) => {}
        }

        let now = Instant::now();
        if now >= status_at {
            report.print_status();
            status_at = now + STATUS_INTERVAL;
        }
        if now >= end {
            break;
        }
    }

    voice.leave(guild_id).await?;
    // Give the gateway time to send the leave before the process ends.
    sleep_until(Instant::now() + Duration::from_secs(1)).await;
    event_task.abort();

    report.print_summary();
    report.result()
}

/// Gives events to the voice manager until a person joins a voice channel of the guild and is
/// alone in it. People who are in a channel at the start do not count. Gives the channel and the
/// person. With `only_channel`, only that channel counts, and also a person who is alone in it
/// at the start.
async fn wait_for_person(
    gateway: &mut ShardManager,
    voice: &VoiceManager,
    guild_id: GuildId,
    only_channel: Option<ChannelId>,
) -> Result<(ChannelId, UserId), BoxError> {
    let deadline = Instant::now() + WAIT_FOR_PERSON;
    let mut own_id = None;
    let mut occupants: HashMap<UserId, ChannelId> = HashMap::new();
    println!("waiting for a person to join an empty voice channel of guild {guild_id}");
    loop {
        let Ok(Some(message)) = timeout_at(deadline, gateway.next_event()).await else {
            return Err("no person joined an empty voice channel".into());
        };
        let event = match &message.event {
            ShardEvent::Dispatch(event) => event,
            ShardEvent::DecodeError { name, error } => {
                println!("decode error: {name}: {error}");
                continue;
            }
            _ => continue,
        };
        voice.handle_event(event);

        let state = match event.as_ref() {
            Event::Ready(ready) => {
                own_id = Some(ready.user.id);
                continue;
            }
            Event::GuildCreate(create) => {
                if let GuildCreate::Available(guild) = create.as_ref()
                    && guild.id == guild_id
                {
                    for state in guild.voice_states.iter().flatten() {
                        if let Some(channel_id) = state.channel_id {
                            occupants.insert(state.user_id, channel_id);
                        }
                    }
                    println!("in voice channels at the start: {occupants:?}");
                    if let Some(channel_id) = only_channel {
                        let people: Vec<UserId> = occupants
                            .iter()
                            .filter(|&(_, &other)| other == channel_id)
                            .map(|(&user_id, _)| user_id)
                            .collect();
                        if let [person] = people[..] {
                            return Ok((channel_id, person));
                        }
                    }
                }
                continue;
            }
            Event::VoiceStateUpdate(update) if update.0.guild_id == Some(guild_id) => &update.0,
            _ => continue,
        };

        let previous = match state.channel_id {
            Some(channel_id) => occupants.insert(state.user_id, channel_id),
            None => occupants.remove(&state.user_id),
        };
        println!(
            "voice state: user {} in channel {:?}",
            state.user_id, state.channel_id
        );
        let Some(channel_id) = state.channel_id else {
            continue;
        };
        if only_channel.is_some_and(|only| only != channel_id) {
            continue;
        }
        // A mute or a deafen in the same channel is not a join.
        if Some(state.user_id) == own_id || previous == Some(channel_id) {
            continue;
        }

        let people = occupants
            .values()
            .filter(|&&other| other == channel_id)
            .count();
        if people == 1 {
            return Ok((channel_id, state.user_id));
        }
        println!("a person joined channel {channel_id}, but it has {people} people; waiting");
    }
}

/// Gives the test tone as interleaved stereo samples: C5, E5, and G5 for 250 ms each, then
/// 750 ms of silence.
fn test_tone(seconds: u64) -> Vec<f32> {
    const NOTES: [f32; 3] = [523.25, 659.25, 783.99];
    const NOTE_FRAMES: usize = 12_000;
    const CYCLE_FRAMES: usize = 72_000;
    const FADE_FRAMES: f32 = 480.0;

    let frames = usize::try_from(seconds * 48_000).expect("the tone fits in memory");
    let mut samples = Vec::with_capacity(frames * 2);
    for index in 0..frames {
        let in_cycle = index % CYCLE_FRAMES;
        let note = in_cycle / NOTE_FRAMES;
        let value = NOTES.get(note).map_or(0.0, |frequency| {
            // A short fade at each edge of a note prevents clicks.
            let in_note = (in_cycle % NOTE_FRAMES) as f32;
            let fade = (in_note / FADE_FRAMES)
                .min((NOTE_FRAMES as f32 - in_note) / FADE_FRAMES)
                .min(1.0);
            let time = in_cycle as f32 / SAMPLE_RATE;
            0.2 * fade * (std::f32::consts::TAU * frequency * time).sin()
        });
        samples.push(value);
        samples.push(value);
    }
    samples
}

/// A loudness meter (EBU R128): integrated and short-term loudness, and the true peak.
struct Loudness {
    meter: EbuR128,
}

impl Loudness {
    fn new() -> Self {
        let mode = Mode::I | Mode::S | Mode::TRUE_PEAK;
        let meter = EbuR128::new(2, 48_000, mode).expect("a stereo meter at 48 kHz");
        Self { meter }
    }

    /// Measures interleaved stereo samples in -1.0 to 1.0.
    fn of_samples(samples: &[f32]) -> Self {
        let mut loudness = Self::new();
        loudness
            .meter
            .add_frames_f32(samples)
            .expect("whole stereo frames");
        loudness
    }

    fn add(&mut self, pcm: &[i16]) {
        // A frame always has whole stereo pairs, so the meter accepts it.
        let _ = self.meter.add_frames_i16(pcm);
    }

    fn short_term(&self) -> String {
        lufs_text(self.meter.loudness_shortterm())
    }

    /// Gives the integrated loudness and the highest true peak of both channels.
    fn describe(&self) -> String {
        let true_peak = (0..2)
            .filter_map(|channel| self.meter.true_peak(channel).ok())
            .fold(0.0_f64, f64::max);
        format!(
            "integrated {}, true peak {:.1} dBTP",
            lufs_text(self.meter.loudness_global()),
            20.0 * true_peak.log10()
        )
    }
}

fn lufs_text(loudness: Result<f64, ebur128::Error>) -> String {
    match loudness {
        Ok(value) if value.is_finite() => format!("{value:.1} LUFS"),
        Ok(_) => "silent".to_owned(),
        Err(error) => format!("unknown ({error})"),
    }
}

/// The received audio of one person.
struct Received {
    frames: u64,
    lost: u64,
    /// The highest absolute sample since the last status line.
    peak: i16,
    /// The highest absolute sample of the test.
    highest_peak: i16,
    loudness: Loudness,
}

impl Default for Received {
    fn default() -> Self {
        Self {
            frames: 0,
            lost: 0,
            peak: 0,
            highest_peak: 0,
            loudness: Loudness::new(),
        }
    }
}

/// The counts of the call.
#[derive(Default)]
struct Report {
    dave_protocol_version: Option<u16>,
    received: BTreeMap<String, Received>,
    lifecycle: Vec<String>,
    e2ee_errors: Vec<String>,
}

impl Report {
    fn add(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Ready {
                dave_protocol_version,
                mode,
                ..
            } => {
                println!("ready: mode {mode:?}, DAVE protocol version {dave_protocol_version}");
                self.dave_protocol_version = Some(dave_protocol_version);
                self.lifecycle.push("Ready".to_owned());
            }
            VoiceEvent::Audio(data) => {
                let sender = data
                    .user_id
                    .map_or_else(|| format!("ssrc {}", data.ssrc), |user| user.to_string());
                let received = self.received.entry(sender).or_default();
                if data.is_lost() {
                    received.lost += 1;
                    return;
                }
                received.frames += 1;
                let peak = data
                    .pcm
                    .iter()
                    .flatten()
                    .map(|sample| sample.saturating_abs())
                    .max()
                    .unwrap_or(0);
                received.peak = received.peak.max(peak);
                received.highest_peak = received.highest_peak.max(peak);
                if let Some(pcm) = &data.pcm {
                    received.loudness.add(pcm);
                }
            }
            VoiceEvent::E2eeError(error) => {
                println!("DAVE error: {error}");
                self.e2ee_errors.push(error);
            }
            VoiceEvent::ClientsConnect(users) => {
                println!("joined the call: {users:?}");
                self.lifecycle
                    .push(format!("ClientsConnect({})", users.len()));
            }
            VoiceEvent::ClientDisconnect(user) => {
                println!("left the call: {user}");
                self.lifecycle.push("ClientDisconnect".to_owned());
            }
            VoiceEvent::Track(event) => println!("track: {event:?}"),
            VoiceEvent::Speaking {
                user_id,
                ssrc,
                flags,
            } => println!("speaking: user {user_id}, ssrc {ssrc}, flags {flags:?}"),
            VoiceEvent::Connecting => {}
            other => {
                println!("call: {other:?}");
                self.lifecycle.push(format!("{other:?}"));
            }
        }
    }

    fn print_status(&mut self) {
        if self.received.is_empty() {
            println!("status: no audio received yet");
        }
        for (sender, received) in &mut self.received {
            println!(
                "status: {sender}: {} frames, {} lost, level {}, short-term {}",
                received.frames,
                received.lost,
                level_bar(received.peak),
                received.loudness.short_term()
            );
            received.peak = 0;
        }
    }

    fn print_summary(&self) {
        println!("summary:");
        println!("  DAVE protocol version: {:?}", self.dave_protocol_version);
        println!("  lifecycle: {:?}", self.lifecycle);
        println!("  DAVE errors: {:?}", self.e2ee_errors);
        for (sender, received) in &self.received {
            println!(
                "  {sender}: {} frames, {} lost, highest peak {}, {}",
                received.frames,
                received.lost,
                received.highest_peak,
                received.loudness.describe()
            );
        }
    }

    fn result(&self) -> Result<(), BoxError> {
        if self.dave_protocol_version != Some(1) {
            return Err("the call did not use DAVE protocol version 1".into());
        }
        if !self.e2ee_errors.is_empty() {
            return Err(format!("{} DAVE error(s)", self.e2ee_errors.len()).into());
        }
        Ok(())
    }
}

/// Shows a peak as a bar of 20 characters.
fn level_bar(peak: i16) -> String {
    let filled = (usize::try_from(peak).unwrap_or(0) * 20) / usize::from(i16::MAX.unsigned_abs());
    format!("[{}{}] {peak}", "#".repeat(filled), " ".repeat(20 - filled))
}
