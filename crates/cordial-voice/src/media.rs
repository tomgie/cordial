//! The media tasks: the 20 ms send clock and the receive loop.

use std::hash::{BuildHasher, RandomState};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::MissedTickBehavior;

#[cfg(feature = "opus")]
use crate::audio::codec::OpusEncoder;
use crate::audio::mixer::{MixOutput, Mixer};
use crate::audio::{FRAME_DURATION, FRAME_SAMPLES, SILENCE_FRAME, SILENCE_FRAMES_BEFORE_PAUSE};
use crate::call::{VoiceEvent, lock};
use crate::core::{SharedE2ee, lock_e2ee};
use crate::crypto::TransportCipher;
use crate::driver::SpeakingRequest;
use crate::packet::RtpHeader;
use crate::protocol::SpeakingFlags;
use crate::receive::Receiver;

const SPEAKING_TIMEOUT: Duration = Duration::from_secs(1);

/// The send task: each 20 ms it takes a frame from the mixer and sends one RTP packet.
pub(crate) struct SendTask {
    pub(crate) socket: Arc<UdpSocket>,
    pub(crate) cipher: TransportCipher,
    pub(crate) ssrc: u32,
    pub(crate) mixer: Arc<Mutex<Mixer>>,
    pub(crate) e2ee: SharedE2ee,
    pub(crate) speaking: mpsc::Sender<SpeakingRequest>,
    pub(crate) events: broadcast::Sender<VoiceEvent>,
    pub(crate) flags: SpeakingFlags,
    pub(crate) bitrate: i32,
}

/// The RTP state of the send task.
struct SendState {
    sequence: u16,
    timestamp: u32,
    speaking: bool,
    silence_left: u8,
    #[cfg(feature = "opus")]
    encoder: Option<OpusEncoder>,
    warned: bool,
}

impl SendTask {
    pub(crate) async fn run(mut self) {
        // RTP starts the sequence number and the timestamp at random values.
        let random = RandomState::new().hash_one(self.ssrc);
        let [sequence_high, sequence_low, timestamp @ ..] = random.to_be_bytes();
        let mut state = SendState {
            sequence: u16::from_be_bytes([sequence_high, sequence_low]),
            timestamp: u32::from_be_bytes([timestamp[0], timestamp[1], timestamp[2], timestamp[3]]),
            speaking: false,
            silence_left: 0,
            #[cfg(feature = "opus")]
            encoder: None,
            warned: false,
        };

        // Burst sends the missed frames after a delay, so the clock does not drift.
        let mut clock = tokio::time::interval(FRAME_DURATION);
        clock.set_missed_tick_behavior(MissedTickBehavior::Burst);
        loop {
            clock.tick().await;
            self.step(&mut state).await;
        }
    }

    async fn step(&mut self, state: &mut SendState) {
        let (output, events) = {
            let mut mixer = lock(&self.mixer);
            (mixer.next_frame(), mixer.take_events())
        };
        for event in events {
            let _ = self.events.send(VoiceEvent::Track(event));
        }

        let (opus, samples) = match output {
            MixOutput::Opus { packet, samples } => (packet, samples),
            MixOutput::Pcm(pcm) => match self.encode(state, &pcm) {
                Some(packet) => (packet, frame_samples()),
                None => return,
            },
            MixOutput::Busy => return,
            MixOutput::Idle => {
                self.send_silence(state).await;
                return;
            }
        };

        if !state.speaking {
            self.set_speaking(self.flags).await;
            state.speaking = true;
        }
        let payload = match lock_e2ee(&self.e2ee).encrypt(&opus) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(%error, "cannot E2EE-encrypt a frame; dropped");
                return;
            }
        };
        self.send_packet(state, &payload, samples).await;
        state.silence_left = SILENCE_FRAMES_BEFORE_PAUSE;
    }

    /// Sends one silence frame before a pause. After the last one, clears the speaking flags.
    async fn send_silence(&mut self, state: &mut SendState) {
        if state.silence_left == 0 {
            return;
        }
        // The silence frame goes out without E2EE: receivers pass it through.
        self.send_packet(state, &SILENCE_FRAME, frame_samples())
            .await;
        state.silence_left -= 1;
        if state.silence_left == 0 {
            self.set_speaking(SpeakingFlags::NONE).await;
            state.speaking = false;
        }
    }

    async fn send_packet(&mut self, state: &mut SendState, payload: &[u8], samples: u32) {
        let header = RtpHeader::new(state.sequence, state.timestamp, self.ssrc).encode();
        match self.cipher.encrypt(&header, payload) {
            Ok(packet) => {
                if let Err(error) = self.socket.send(&packet).await {
                    tracing::debug!(%error, "cannot send a voice packet");
                }
            }
            Err(error) => tracing::warn!(%error, "cannot encrypt a voice packet"),
        }
        state.sequence = state.sequence.wrapping_add(1);
        state.timestamp = state.timestamp.wrapping_add(samples);
    }

    async fn set_speaking(&self, flags: SpeakingFlags) {
        let (reply, answer) = oneshot::channel();
        if self.speaking.send((flags, reply)).await.is_err() {
            return;
        }
        let _ = tokio::time::timeout(SPEAKING_TIMEOUT, answer).await;
    }

    #[cfg(feature = "opus")]
    fn encode(&self, state: &mut SendState, pcm: &[f32]) -> Option<Vec<u8>> {
        if state.encoder.is_none() {
            let mut encoder = OpusEncoder::new()
                .map_err(|error| tracing::warn!(%error, "cannot make an Opus encoder"))
                .ok()?;
            if let Err(error) = encoder.set_bitrate(self.bitrate) {
                tracing::warn!(%error, "the Opus bitrate is not accepted");
            }
            state.encoder = Some(encoder);
        }
        let encoder = state.encoder.as_mut()?;
        match encoder.encode(pcm) {
            Ok(packet) => Some(packet),
            Err(error) => {
                if !state.warned {
                    tracing::warn!(%error, "cannot encode a frame");
                    state.warned = true;
                }
                None
            }
        }
    }

    #[cfg(not(feature = "opus"))]
    fn encode(&self, state: &mut SendState, _pcm: &[f32]) -> Option<Vec<u8>> {
        if !state.warned {
            tracing::warn!(
                bitrate = self.bitrate,
                "PCM audio needs the `opus` feature of cordial-voice; it is not sent"
            );
            state.warned = true;
        }
        None
    }
}

fn frame_samples() -> u32 {
    u32::try_from(FRAME_SAMPLES).unwrap_or(960)
}

/// The receive task: reads the UDP socket and runs the jitter buffers each 20 ms.
pub(crate) async fn receive_loop(
    socket: Arc<UdpSocket>,
    mut receiver: Receiver,
    events: broadcast::Sender<VoiceEvent>,
) {
    let mut clock = tokio::time::interval(FRAME_DURATION);
    clock.set_missed_tick_behavior(MissedTickBehavior::Burst);
    let mut buffer = vec![0_u8; 2048];
    loop {
        tokio::select! {
            result = socket.recv(&mut buffer) => match result {
                Ok(length) => receiver.push_packet(&buffer[..length]),
                Err(error) => {
                    // Windows reports ICMP errors on UDP receive. Wait a moment and go on.
                    tracing::debug!(%error, "voice receive error");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            },
            _ = clock.tick() => {
                for data in receiver.tick() {
                    let _ = events.send(VoiceEvent::Audio(Box::new(data)));
                }
            }
        }
    }
}
