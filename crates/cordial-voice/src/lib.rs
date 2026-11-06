//! Discord voice connections.
//!
//! This crate contains the voice gateway (version 8), the UDP transport, transport
//! encryption, the interface to DAVE end-to-end encryption, and the audio pipeline.
//! See `docs/ARCHITECTURE.md`, section 11.
//!
//! | Part | Module |
//! |---|---|
//! | Voice gateway payloads, opcodes, close codes | [`protocol`] |
//! | Sans-IO protocol core | [`core`] |
//! | IP discovery, RTP | [`packet`] |
//! | Transport encryption (rtpsize AEAD modes) | [`crypto`] |
//! | E2EE interface | [`e2ee`] |
//! | DAVE end-to-end encryption | [`dave`] |
//! | Sources (PCM, Ogg Opus, external programs), mixer, Opus codec | [`audio`] |
//! | Jitter buffer, receive pipeline | [`receive`] |
//! | Tokio driver and the call handle | [`Call`] |
//! | Join and leave through the main gateway | [`VoiceManager`] |
//!
//! # Features
//!
//! - `opus`: Opus encode and decode with libopus. It is necessary to send PCM and to decode
//!   received audio. It needs CMake and a C compiler. Ogg Opus passthrough works without it.
//! - `symphonia`: decode MP3, FLAC, WAV, Vorbis, and AAC files. It turns on `opus`.
//!
//! # Example
//!
//! ```no_run
//! # async fn run(shard: cordial_gateway::ShardHandle, guild_id: cordial_model::id::GuildId,
//! #     channel_id: cordial_model::id::ChannelId) -> Result<(), Box<dyn std::error::Error>> {
//! use cordial_voice::{
//!     CallConfig, DaveSession, E2eeSession, OggOpusSource, Track, VoiceEvent, VoiceManager,
//! };
//!
//! // Discord needs DAVE end-to-end encryption: one session for each call.
//! let manager = VoiceManager::with_e2ee(shard, CallConfig::default(), || {
//!     Box::new(DaveSession::new().expect("the DAVE key pair is made")) as Box<dyn E2eeSession>
//! });
//! // Give each gateway event to the manager: manager.handle_event(&event);
//! let call = manager.join(guild_id, channel_id).await?;
//! let track = call.enqueue(Track::new(OggOpusSource::open("intro.ogg")?));
//! let mut events = call.subscribe();
//! while let Ok(event) = events.recv().await {
//!     if let VoiceEvent::Track(track_event) = event {
//!         println!("{track_event:?}");
//!     }
//! }
//! track.stop();
//! manager.leave(guild_id).await?;
//! # Ok(())
//! # }
//! ```

pub mod audio;
mod call;
pub mod core;
pub mod crypto;
pub mod dave;
mod driver;
pub mod e2ee;
mod manager;
mod media;
pub mod packet;
pub mod protocol;
pub mod receive;

pub use audio::mixer::{
    EndReason, MixOutput, Mixer, PlayState, Track, TrackEvent, TrackHandle, TrackId,
};
pub use audio::ogg::OggOpusSource;
pub use audio::pcm::PcmSource;
pub use audio::process::ProcessSource;
pub use audio::{AudioError, AudioSource, SourceFrame};
pub use call::{Call, CallConfig, CallError, CallState, VoiceEvent};
pub use core::ConnectionInfo;
pub use dave::DaveSession;
pub use e2ee::{E2eeSession, NoE2ee};
pub use manager::{
    E2eeFactory, GatewayFuture, JoinError, ShardHandles, VoiceGateway, VoiceManager,
};
pub use protocol::{EncryptionMode, SpeakingFlags, VoiceCloseCode};
pub use receive::VoiceData;
