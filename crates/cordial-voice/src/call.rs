//! A voice call: the handle that a bot uses to play audio and to receive events.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use cordial_model::id::{ChannelId, GuildId, SoundboardSoundId, UserId};
use tokio::sync::{broadcast, mpsc, watch};

use crate::audio::mixer::{Mixer, Track, TrackEvent, TrackHandle};
use crate::core::{ConnectionInfo, VoiceFatal};
use crate::driver::{Driver, DriverCommand};
use crate::e2ee::E2eeSession;
use crate::protocol::{EncryptionMode, SpeakingFlags};
use crate::receive::{DEFAULT_JITTER_DEPTH, VoiceData};

/// The settings of a call.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct CallConfig {
    /// Tells if the call receives audio and gives [`VoiceEvent::Audio`] events.
    pub receive: bool,
    /// Tells if received audio is decoded to PCM. Needs the `opus` feature.
    pub decode: bool,
    /// The number of packets that the jitter buffer holds before playout.
    pub jitter_depth: usize,
    /// Joins muted (opcode 4 of the main gateway).
    pub self_mute: bool,
    /// Joins deafened (opcode 4 of the main gateway).
    pub self_deaf: bool,
    /// The time to wait for the voice events and for the voice session to become ready.
    pub timeout: Duration,
    /// The speaking flags to send before audio.
    pub speaking: SpeakingFlags,
    /// The capacity of the event channel. A slow receiver misses old events.
    pub event_buffer: usize,
    /// The Opus bitrate in bits per second, for PCM sources.
    pub bitrate: i32,
}

impl Default for CallConfig {
    fn default() -> Self {
        Self {
            receive: false,
            decode: true,
            jitter_depth: DEFAULT_JITTER_DEPTH,
            self_mute: false,
            self_deaf: false,
            timeout: Duration::from_secs(10),
            speaking: SpeakingFlags::MICROPHONE,
            event_buffer: 256,
            bitrate: 64_000,
        }
    }
}

impl CallConfig {
    /// Turns audio receive on or off.
    #[must_use]
    pub fn receive(mut self, receive: bool) -> Self {
        self.receive = receive;
        self
    }

    /// Turns the decode of received audio on or off.
    #[must_use]
    pub fn decode(mut self, decode: bool) -> Self {
        self.decode = decode;
        self
    }

    /// Sets the time to wait for the voice events and the voice session.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the mute and deaf flags of the bot.
    #[must_use]
    pub fn self_mute_deaf(mut self, mute: bool, deaf: bool) -> Self {
        self.self_mute = mute;
        self.self_deaf = deaf;
        self
    }

    /// Sets the Opus bitrate in bits per second.
    #[must_use]
    pub fn bitrate(mut self, bitrate: i32) -> Self {
        self.bitrate = bitrate;
        self
    }
}

/// An event of a call.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum VoiceEvent {
    /// The call connects to the voice gateway.
    Connecting,
    /// The voice session is ready. Audio flows.
    Ready {
        /// The SSRC of the bot.
        ssrc: u32,
        /// The transport encryption mode.
        mode: EncryptionMode,
        /// The DAVE protocol version.
        dave_protocol_version: u16,
    },
    /// The voice session resumed.
    Resumed,
    /// The voice gateway connection closed. The call connects again when it can.
    Disconnected {
        /// The close code, if one arrived.
        code: Option<u16>,
    },
    /// The speaking state of a user changed.
    Speaking {
        /// The user.
        user_id: UserId,
        /// The SSRC of the user.
        ssrc: u32,
        /// The speaking flags.
        flags: SpeakingFlags,
    },
    /// Users joined the call.
    ClientsConnect(Vec<UserId>),
    /// A user left the call.
    ClientDisconnect(UserId),
    /// 20 ms of received audio from one user. Needs [`CallConfig::receive`].
    Audio(Box<VoiceData>),
    /// A track started or ended.
    Track(TrackEvent),
    /// The E2EE session reported an error.
    E2eeError(String),
    /// The call stopped.
    Stopped(Option<VoiceFatal>),
}

/// The state of a call.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallState {
    /// The call connects.
    Connecting,
    /// The voice session is ready.
    Ready,
    /// The call connects again after a lost connection.
    Reconnecting,
    /// The voice server moves; the call waits for a new server.
    WaitingForServer,
    /// The call stopped, with the error if there is one.
    Stopped(Option<VoiceFatal>),
}

/// An error of a call.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CallError {
    /// The voice session did not become ready in time.
    #[error("the voice session did not become ready in time")]
    Timeout,
    /// The call stopped.
    #[error("the call stopped{}", .0.as_ref().map(|error| format!(": {error}")).unwrap_or_default())]
    Stopped(Option<VoiceFatal>),
}

pub(crate) struct CallShared {
    pub(crate) guild_id: GuildId,
    pub(crate) info: Mutex<ConnectionInfo>,
    pub(crate) commands: mpsc::UnboundedSender<DriverCommand>,
    pub(crate) mixer: Arc<Mutex<Mixer>>,
    pub(crate) events: broadcast::Sender<VoiceEvent>,
    pub(crate) state: Arc<watch::Sender<CallState>>,
    pub(crate) latency: Arc<Mutex<Option<Duration>>>,
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A voice call in one guild. It is cheap to clone. All clones control the same call.
///
/// The call connects again after a lost connection. It stops after [`Call::disconnect`],
/// after a fatal close code, or when all clones are dropped.
///
/// [`VoiceManager::join`](crate::VoiceManager::join) gives a call.
///
/// ```no_run
/// # async fn run(call: cordial_voice::Call) -> Result<(), Box<dyn std::error::Error>> {
/// use std::time::Duration;
///
/// use cordial_voice::{OggOpusSource, Track, TrackEvent, VoiceEvent};
///
/// // Two tracks in the queue. The second starts when the first ends.
/// let intro = call.enqueue(Track::new(OggOpusSource::open("intro.ogg")?).volume(0.8));
/// let song = call.enqueue(OggOpusSource::open("song.ogg")?);
/// intro.seek(Duration::from_secs(5));
///
/// let mut events = call.subscribe();
/// while let Ok(event) = events.recv().await {
///     if let VoiceEvent::Track(TrackEvent::Ended { track, .. }) = event
///         && track == song.id()
///     {
///         break;
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Call {
    shared: Arc<CallShared>,
}

impl fmt::Debug for Call {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Call")
            .field("guild_id", &self.shared.guild_id)
            .field("channel_id", &self.channel_id())
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl Call {
    /// Starts a call with connection data from the main gateway. Most bots use
    /// [`VoiceManager::join`](crate::VoiceManager::join) instead.
    ///
    /// It must run inside a Tokio runtime.
    #[must_use]
    pub fn connect(info: ConnectionInfo, config: CallConfig, e2ee: Box<dyn E2eeSession>) -> Self {
        let (commands, command_receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(config.event_buffer.max(1));
        let (state, _) = watch::channel(CallState::Connecting);
        let shared = Arc::new(CallShared {
            guild_id: info.guild_id,
            info: Mutex::new(info.clone()),
            commands,
            mixer: Arc::new(Mutex::new(Mixer::new())),
            events,
            state: Arc::new(state),
            latency: Arc::default(),
        });

        let parts = crate::driver::DriverParts {
            commands: command_receiver,
            mixer: Arc::clone(&shared.mixer),
            events: shared.events.clone(),
            state: Arc::clone(&shared.state),
            latency: Arc::clone(&shared.latency),
        };
        let driver = Driver::new(info, config, e2ee, parts);
        tokio::spawn(driver.run());
        Self { shared }
    }

    /// Gives the guild ID.
    #[must_use]
    pub fn guild_id(&self) -> GuildId {
        self.shared.guild_id
    }

    /// Gives the voice channel ID.
    #[must_use]
    pub fn channel_id(&self) -> ChannelId {
        lock(&self.shared.info).channel_id
    }

    /// Gives the state.
    #[must_use]
    pub fn state(&self) -> CallState {
        self.shared.state.borrow().clone()
    }

    /// Gives the last heartbeat latency of the voice gateway.
    #[must_use]
    pub fn latency(&self) -> Option<Duration> {
        *lock(&self.shared.latency)
    }

    /// Gives a receiver of the events of the call.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<VoiceEvent> {
        self.shared.events.subscribe()
    }

    /// Waits until the voice session is ready.
    ///
    /// # Errors
    ///
    /// Gives an error when the time ends first or the call stops.
    pub async fn wait_ready(&self, timeout: Duration) -> Result<(), CallError> {
        let mut state = self.shared.state.subscribe();
        let wait = async {
            loop {
                match &*state.borrow_and_update() {
                    CallState::Ready => return Ok(()),
                    CallState::Stopped(error) => return Err(CallError::Stopped(error.clone())),
                    _ => {}
                }
                if state.changed().await.is_err() {
                    return Err(CallError::Stopped(None));
                }
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| CallError::Timeout)?
    }

    /// Starts a track now. It plays at the same time as other tracks.
    pub fn play(&self, track: impl Into<Track>) -> TrackHandle {
        lock(&self.shared.mixer).play(track)
    }

    /// Adds a track to the end of the queue.
    pub fn enqueue(&self, track: impl Into<Track>) -> TrackHandle {
        lock(&self.shared.mixer).enqueue(track)
    }

    /// Stops the current track of the queue. The next track starts.
    pub fn skip(&self) {
        lock(&self.shared.mixer).skip();
    }

    /// Stops all tracks and clears the queue.
    pub fn stop_all(&self) {
        lock(&self.shared.mixer).stop_all();
    }

    /// Gives the number of tracks in the queue, with the current queue track.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        lock(&self.shared.mixer).queue_len()
    }

    /// Plays a soundboard sound in the voice channel of the call.
    ///
    /// Set `source_guild_id` for a sound of a different guild.
    ///
    /// # Errors
    ///
    /// Gives the error of the HTTP request.
    pub async fn send_soundboard_sound(
        &self,
        http: &cordial_http::Client,
        sound_id: SoundboardSoundId,
        source_guild_id: Option<GuildId>,
    ) -> Result<(), cordial_http::Error> {
        let mut request = http.send_soundboard_sound(self.channel_id(), sound_id);
        if let Some(guild_id) = source_guild_id {
            request = request.source_guild_id(guild_id);
        }
        request.await
    }

    /// In a stage channel: asks to speak. A stage moderator sees the request.
    ///
    /// # Errors
    ///
    /// Gives the error of the HTTP request.
    pub async fn request_to_speak(
        &self,
        http: &cordial_http::Client,
    ) -> Result<(), cordial_http::Error> {
        http.edit_current_voice_state(self.guild_id())
            .channel_id(self.channel_id())
            .request_to_speak_timestamp(cordial_model::Timestamp::now())
            .await
            .map(drop)
    }

    /// In a stage channel: becomes a speaker (`false`) or an audience member (`true`).
    ///
    /// To become a speaker, the bot needs the `MUTE_MEMBERS` permission in the stage.
    ///
    /// # Errors
    ///
    /// Gives the error of the HTTP request.
    pub async fn set_suppressed(
        &self,
        http: &cordial_http::Client,
        suppressed: bool,
    ) -> Result<(), cordial_http::Error> {
        http.edit_current_voice_state(self.guild_id())
            .channel_id(self.channel_id())
            .suppress(suppressed)
            .await
            .map(drop)
    }

    /// Sets the status text of the voice channel, or removes it with `None`.
    ///
    /// # Errors
    ///
    /// Gives the error of the HTTP request.
    pub async fn set_channel_status(
        &self,
        http: &cordial_http::Client,
        status: Option<&str>,
    ) -> Result<(), cordial_http::Error> {
        let mut request = http.set_voice_channel_status(self.channel_id());
        if let Some(status) = status {
            request = request.status(status);
        }
        request.await.map(drop)
    }

    /// Stops the call and closes the voice connection. It does not change the voice state
    /// in the main gateway; use [`VoiceManager::leave`](crate::VoiceManager::leave) for that.
    pub fn disconnect(&self) {
        let _ = self.shared.commands.send(DriverCommand::Stop);
    }

    /// Connects to a new voice server or session, for example after a channel move.
    pub(crate) fn update_server(&self, info: ConnectionInfo) {
        *lock(&self.shared.info) = info.clone();
        if self.set_state_unless_stopped(CallState::Connecting) {
            let _ = self.shared.commands.send(DriverCommand::Connect(info));
        }
    }

    /// Tells the call that the voice server is being moved.
    pub(crate) fn server_lost(&self) {
        if self.set_state_unless_stopped(CallState::WaitingForServer) {
            let _ = self.shared.commands.send(DriverCommand::ServerLost);
        }
    }

    /// Tells if the call stopped. A stopped call does not connect again.
    pub(crate) fn is_stopped(&self) -> bool {
        self.shared.commands.is_closed()
            || matches!(*self.shared.state.borrow(), CallState::Stopped(_))
    }

    /// Tells if `other` is a clone of this call.
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    /// Sets the state. Gives `false` and keeps the state when the call stopped: the driver
    /// does not run anymore, and the stop must stay visible.
    fn set_state_unless_stopped(&self, state: CallState) -> bool {
        self.shared.state.send_if_modified(|current| {
            if matches!(current, CallState::Stopped(_)) {
                return false;
            }
            *current = state;
            true
        })
    }

    /// Gives the current connection data.
    pub(crate) fn info(&self) -> ConnectionInfo {
        lock(&self.shared.info).clone()
    }
}
