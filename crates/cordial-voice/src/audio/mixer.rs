//! The mixer: tracks, volume, pause, and a track queue.
//!
//! - [`Mixer::play`] starts a track now. It plays at the same time as other tracks.
//! - [`Mixer::enqueue`] adds a track to the queue. Queued tracks play one after another.
//!
//! When one track plays alone at volume 1.0 and gives Opus packets (Ogg Opus), the packets go
//! out without a new encode. In all other cases the mixer adds the PCM of the tracks. Then
//! Opus packets must be decoded, which needs the `opus` feature.
//!
//! The send clock sends one packet each 20 ms. Thus only packets with a duration of a
//! multiple of 20 ms go out without a new encode. The mixer decodes other packets (for
//! example 10 ms packets) and encodes the audio again in 20 ms frames. Without the `opus`
//! feature, a track with such packets ends with an error.
//!
//! [`TrackHandle::seek`] moves a track. The mixer applies the seek before the next frame.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

#[cfg(feature = "opus")]
use super::codec::OpusDecoder;
use super::{
    AudioSource, CHANNELS, FRAME_LEN, FRAME_SAMPLES, SourceFrame, duration_to_samples,
    samples_to_duration,
};

/// The ID of a track in a mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(u64);

impl TrackId {
    /// Gives the raw value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The play state of a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlayState {
    /// The track plays, or waits in the queue.
    Playing,
    /// The track is paused.
    Paused,
    /// The track ended.
    Ended,
}

/// Why a track ended.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EndReason {
    /// The source has no more audio.
    Finished,
    /// The track was stopped.
    Stopped,
    /// The source gave an error.
    Error(String),
}

/// An event of a track.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrackEvent {
    /// The track started to play.
    Started(TrackId),
    /// The track ended.
    Ended {
        /// The track.
        track: TrackId,
        /// Why the track ended.
        reason: EndReason,
    },
    /// A seek of the track is complete.
    Seeked {
        /// The track.
        track: TrackId,
        /// The position that the track plays from now. It can be a little before the
        /// requested position.
        position: Duration,
    },
    /// A seek of the track failed, for example because its source cannot seek. The track
    /// plays on from its old position.
    SeekFailed {
        /// The track.
        track: TrackId,
        /// The error.
        error: String,
    },
}

#[derive(Debug)]
struct Control {
    state: PlayState,
    volume: f32,
    stop: bool,
    seek: Option<Duration>,
    /// The number of samples for each channel that the track gave to the mixer output.
    played: u64,
}

fn lock(control: &Mutex<Control>) -> MutexGuard<'_, Control> {
    control.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A handle to control a track. It is cheap to clone.
#[derive(Clone)]
pub struct TrackHandle {
    id: TrackId,
    control: Arc<Mutex<Control>>,
}

impl fmt::Debug for TrackHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrackHandle")
            .field("id", &self.id)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl TrackHandle {
    /// Gives the track ID.
    #[must_use]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Pauses the track.
    pub fn pause(&self) {
        let mut control = lock(&self.control);
        if control.state == PlayState::Playing {
            control.state = PlayState::Paused;
        }
    }

    /// Continues a paused track.
    pub fn resume(&self) {
        let mut control = lock(&self.control);
        if control.state == PlayState::Paused {
            control.state = PlayState::Playing;
        }
    }

    /// Stops the track. The mixer removes it on the next frame.
    pub fn stop(&self) {
        lock(&self.control).stop = true;
    }

    /// Sets the volume: 1.0 is the original volume, 0.0 is silent.
    pub fn set_volume(&self, volume: f32) {
        let volume = if volume.is_finite() {
            volume.max(0.0)
        } else {
            1.0
        };
        lock(&self.control).volume = volume;
    }

    /// Gives the volume.
    #[must_use]
    pub fn volume(&self) -> f32 {
        lock(&self.control).volume
    }

    /// Gives the play state.
    #[must_use]
    pub fn state(&self) -> PlayState {
        lock(&self.control).state
    }

    /// Moves the track to a position from the start of its audio. The mixer applies the
    /// seek before the next frame and sends [`TrackEvent::Seeked`] or
    /// [`TrackEvent::SeekFailed`].
    ///
    /// The precision depends on the source. See [`AudioSource::seek`].
    pub fn seek(&self, position: Duration) {
        lock(&self.control).seek = Some(position);
    }

    /// Gives the position of the track: the duration of the audio that the track gave to the
    /// mixer since its start or its last seek.
    #[must_use]
    pub fn position(&self) -> Duration {
        samples_to_duration(lock(&self.control).played)
    }
}

/// A track: an audio source with its start settings.
pub struct Track {
    source: Box<dyn AudioSource>,
    volume: f32,
    paused: bool,
}

impl fmt::Debug for Track {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Track")
            .field("volume", &self.volume)
            .field("paused", &self.paused)
            .finish_non_exhaustive()
    }
}

impl Track {
    /// Makes a track from a source at volume 1.0.
    #[must_use]
    pub fn new(source: impl AudioSource) -> Self {
        Self {
            source: Box::new(source),
            volume: 1.0,
            paused: false,
        }
    }

    /// Sets the start volume.
    #[must_use]
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = if volume.is_finite() {
            volume.max(0.0)
        } else {
            1.0
        };
        self
    }

    /// Makes the track start paused.
    #[must_use]
    pub fn paused(mut self) -> Self {
        self.paused = true;
        self
    }
}

impl<S: AudioSource> From<S> for Track {
    fn from(source: S) -> Self {
        Self::new(source)
    }
}

/// The output of the mixer for one 20 ms step.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MixOutput {
    /// An Opus packet to send without a new encode.
    Opus {
        /// The packet.
        packet: Vec<u8>,
        /// Its duration in samples for each channel.
        samples: u32,
    },
    /// 20 ms of mixed PCM to encode and send.
    Pcm(Vec<f32>),
    /// A long Opus packet from an earlier step still plays. Send nothing.
    Busy,
    /// No track plays.
    Idle,
}

struct Active {
    id: TrackId,
    control: Arc<Mutex<Control>>,
    source: Box<dyn AudioSource>,
    queued: bool,
    backlog: VecDeque<f32>,
    wait: u32,
    source_done: bool,
    error: Option<String>,
    #[cfg(not(feature = "opus"))]
    warned: bool,
    #[cfg(feature = "opus")]
    decoder: Option<OpusDecoder>,
}

impl Active {
    fn new(id: TrackId, control: Arc<Mutex<Control>>, track: Track, queued: bool) -> Self {
        Self {
            id,
            control,
            source: track.source,
            queued,
            backlog: VecDeque::new(),
            wait: 0,
            source_done: false,
            error: None,
            #[cfg(not(feature = "opus"))]
            warned: false,
            #[cfg(feature = "opus")]
            decoder: None,
        }
    }

    fn pull(&mut self) -> Option<SourceFrame> {
        if self.source_done {
            return None;
        }
        match self.source.next_frame() {
            Ok(Some(frame)) => Some(frame),
            Ok(None) => {
                self.source_done = true;
                None
            }
            Err(error) => {
                tracing::warn!(track = self.id.0, %error, "an audio source failed");
                self.error = Some(error.to_string());
                self.source_done = true;
                None
            }
        }
    }

    /// Reads frames until the backlog has one frame or the source ends. Gives `false` when
    /// the source waits for audio and the backlog has less than one frame.
    fn fill_backlog(&mut self) -> bool {
        while self.backlog.len() < FRAME_LEN {
            match self.pull() {
                Some(SourceFrame::Pcm(samples)) => self.backlog.extend(samples),
                Some(SourceFrame::Opus {
                    packet,
                    samples,
                    discard_start,
                    discard_end,
                }) => self.decode_into_backlog(&packet, samples, discard_start, discard_end),
                Some(SourceFrame::Waiting) => return false,
                None => return true,
            }
        }
        true
    }

    /// Applies a seek and gives the event for it.
    fn seek(&mut self, position: Duration) -> TrackEvent {
        match self.source.seek(position) {
            Ok(actual) => {
                self.backlog.clear();
                self.wait = 0;
                self.source_done = false;
                self.error = None;
                // The decoder state belongs to the old position.
                #[cfg(feature = "opus")]
                {
                    self.decoder = None;
                }
                lock(&self.control).played = duration_to_samples(actual);
                TrackEvent::Seeked {
                    track: self.id,
                    position: actual,
                }
            }
            Err(error) => {
                tracing::debug!(track = self.id.0, %error, "a track cannot seek");
                TrackEvent::SeekFailed {
                    track: self.id,
                    error: error.to_string(),
                }
            }
        }
    }

    #[cfg(feature = "opus")]
    fn decode_into_backlog(
        &mut self,
        packet: &[u8],
        _samples: u32,
        discard_start: u32,
        discard_end: u32,
    ) {
        if self.decoder.is_none() {
            match OpusDecoder::new() {
                Ok(decoder) => self.decoder = Some(decoder),
                Err(error) => {
                    self.error = Some(error.to_string());
                    self.source_done = true;
                    return;
                }
            }
        }
        let Some(decoder) = self.decoder.as_mut() else {
            return;
        };
        match decoder.decode_float(packet) {
            Ok(pcm) => {
                let decoded = pcm.len() / CHANNELS;
                let start = usize::try_from(discard_start)
                    .unwrap_or(usize::MAX)
                    .min(decoded);
                let end = decoded
                    .saturating_sub(usize::try_from(discard_end).unwrap_or(usize::MAX))
                    .max(start);
                self.backlog.extend(&pcm[start * CHANNELS..end * CHANNELS]);
            }
            Err(error) => tracing::debug!(%error, "cannot decode an Opus packet of a track"),
        }
    }

    #[cfg(not(feature = "opus"))]
    fn decode_into_backlog(
        &mut self,
        _packet: &[u8],
        samples: u32,
        discard_start: u32,
        discard_end: u32,
    ) {
        if !self.warned {
            tracing::warn!(
                track = self.id.0,
                "an Opus track cannot be mixed without the `opus` feature; it is silent"
            );
            self.warned = true;
        }
        let kept = samples
            .saturating_sub(discard_start)
            .saturating_sub(discard_end);
        let silent = usize::try_from(kept).unwrap_or(FRAME_SAMPLES) * CHANNELS;
        self.backlog.extend(std::iter::repeat_n(0.0, silent));
    }

    /// Handles an Opus packet that is not a multiple of 20 ms: the send clock cannot send
    /// it without a new encode.
    #[cfg(feature = "opus")]
    fn take_unaligned_packet(&mut self, packet: &[u8], samples: u32, discard: (u32, u32)) {
        self.decode_into_backlog(packet, samples, discard.0, discard.1);
    }

    #[cfg(not(feature = "opus"))]
    fn take_unaligned_packet(&mut self, _packet: &[u8], samples: u32, _discard: (u32, u32)) {
        tracing::warn!(
            track = self.id.0,
            samples,
            "Opus packets that are not a multiple of 20 ms need the `opus` feature; the track is skipped"
        );
        self.error = Some(
            super::AudioError::Unsupported(format!(
                "Opus packets of {samples} samples need the `opus` feature"
            ))
            .to_string(),
        );
        self.source_done = true;
    }

    fn can_pass_through(&self, volume: f32) -> bool {
        let unity = (volume - 1.0).abs() < f32::EPSILON;
        self.backlog.is_empty() && (unity || !cfg!(feature = "opus"))
    }

    fn end_reason(&self) -> EndReason {
        self.error
            .clone()
            .map_or(EndReason::Finished, EndReason::Error)
    }
}

/// The mixer of a call.
pub struct Mixer {
    active: Vec<Active>,
    queue: VecDeque<Active>,
    events: Vec<TrackEvent>,
    next_id: u64,
}

impl fmt::Debug for Mixer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Mixer")
            .field("active", &self.active.len())
            .field("queue", &self.queue.len())
            .finish_non_exhaustive()
    }
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    /// Makes an empty mixer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            queue: VecDeque::new(),
            events: Vec::new(),
            next_id: 0,
        }
    }

    fn make(&mut self, track: Track, queued: bool) -> (TrackHandle, Active) {
        self.next_id += 1;
        let id = TrackId(self.next_id);
        let state = if track.paused {
            PlayState::Paused
        } else {
            PlayState::Playing
        };
        let control = Arc::new(Mutex::new(Control {
            state,
            volume: track.volume,
            stop: false,
            seek: None,
            played: 0,
        }));
        let handle = TrackHandle {
            id,
            control: Arc::clone(&control),
        };
        (handle, Active::new(id, control, track, queued))
    }

    /// Starts a track now, at the same time as other tracks.
    pub fn play(&mut self, track: impl Into<Track>) -> TrackHandle {
        let (handle, active) = self.make(track.into(), false);
        self.events.push(TrackEvent::Started(active.id));
        self.active.push(active);
        handle
    }

    /// Adds a track to the end of the queue.
    pub fn enqueue(&mut self, track: impl Into<Track>) -> TrackHandle {
        let (handle, active) = self.make(track.into(), true);
        self.queue.push_back(active);
        self.promote();
        handle
    }

    /// Stops the current track of the queue. The next track starts.
    pub fn skip(&mut self) {
        if let Some(current) = self.active.iter().find(|track| track.queued) {
            lock(&current.control).stop = true;
        }
    }

    /// Stops all tracks and clears the queue.
    pub fn stop_all(&mut self) {
        for track in self.active.iter().chain(self.queue.iter()) {
            lock(&track.control).stop = true;
        }
        self.remove_stopped();
    }

    /// Gives the number of tracks in the queue, with the current queue track.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len() + self.active.iter().filter(|track| track.queued).count()
    }

    /// Tells if no track plays or waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.queue.is_empty()
    }

    /// Takes the track events since the last call.
    pub fn take_events(&mut self) -> Vec<TrackEvent> {
        std::mem::take(&mut self.events)
    }

    /// Makes the output for the next 20 ms.
    pub fn next_frame(&mut self) -> MixOutput {
        self.remove_stopped();
        self.promote();
        self.apply_seeks();
        let output = self.produce();
        // When the current queue track ended, the next one starts without a gap.
        if output == MixOutput::Idle && self.promote() {
            return self.produce();
        }
        output
    }

    /// Starts the next queue track when no queue track plays. Gives `true` when one started.
    fn promote(&mut self) -> bool {
        if self.active.iter().any(|track| track.queued) {
            return false;
        }
        let Some(next) = self.queue.pop_front() else {
            return false;
        };
        self.events.push(TrackEvent::Started(next.id));
        self.active.push(next);
        true
    }

    fn apply_seeks(&mut self) {
        for track in self.active.iter_mut().chain(self.queue.iter_mut()) {
            let Some(position) = lock(&track.control).seek.take() else {
                continue;
            };
            self.events.push(track.seek(position));
        }
    }

    fn remove_stopped(&mut self) {
        let events = &mut self.events;
        let mut keep = |track: &Active| {
            let mut control = lock(&track.control);
            if !control.stop {
                return true;
            }
            control.state = PlayState::Ended;
            events.push(TrackEvent::Ended {
                track: track.id,
                reason: EndReason::Stopped,
            });
            false
        };
        self.active.retain(&mut keep);
        self.queue.retain(&mut keep);
    }

    fn produce(&mut self) -> MixOutput {
        let mut ready = Vec::new();
        let mut busy = 0;
        for (index, track) in self.active.iter_mut().enumerate() {
            if lock(&track.control).state != PlayState::Playing {
                continue;
            }
            if track.wait > 0 {
                track.wait -= 1;
                busy += 1;
            } else {
                ready.push(index);
            }
        }

        let output = if let [single] = ready[..]
            && busy == 0
        {
            self.produce_single(single)
        } else {
            self.produce_mix(&ready)
        };

        self.remove_ended();
        match output {
            MixOutput::Idle if busy > 0 => MixOutput::Busy,
            other => other,
        }
    }

    fn produce_single(&mut self, index: usize) -> MixOutput {
        let track = &mut self.active[index];
        let volume = lock(&track.control).volume;
        if !track.can_pass_through(volume) {
            return self.produce_mix(&[index]);
        }

        let frame_samples = u32::try_from(FRAME_SAMPLES).unwrap_or(960);
        match track.pull() {
            Some(SourceFrame::Opus {
                packet,
                samples,
                discard_start,
                discard_end,
            }) => {
                if samples % frame_samples != 0 {
                    track.take_unaligned_packet(&packet, samples, (discard_start, discard_end));
                    return self.produce_mix(&[index]);
                }
                track.wait = (samples / frame_samples).saturating_sub(1);
                // The position follows the decoded time line, so it does not count the
                // samples that a decode drops.
                let kept = samples.saturating_sub(discard_start.saturating_add(discard_end));
                lock(&track.control).played += u64::from(kept);
                MixOutput::Opus { packet, samples }
            }
            Some(SourceFrame::Pcm(samples)) => {
                track.backlog.extend(samples);
                self.produce_mix(&[index])
            }
            Some(SourceFrame::Waiting) | None => MixOutput::Idle,
        }
    }

    fn produce_mix(&mut self, indices: &[usize]) -> MixOutput {
        let mut mix = vec![0.0_f32; FRAME_LEN];
        let mut any = false;
        for &index in indices {
            let track = &mut self.active[index];
            // A track that waits for audio keeps its part of a frame for the next step.
            if !track.fill_backlog() || track.backlog.is_empty() {
                continue;
            }

            let count = track.backlog.len().min(FRAME_LEN);
            let volume = {
                let mut control = lock(&track.control);
                control.played += u64::try_from(count / CHANNELS).unwrap_or(0);
                control.volume
            };
            for (output, sample) in mix.iter_mut().zip(track.backlog.drain(..count)) {
                *output += sample * volume;
            }
            any = true;
        }

        if !any {
            return MixOutput::Idle;
        }
        for sample in &mut mix {
            *sample = sample.clamp(-1.0, 1.0);
        }
        MixOutput::Pcm(mix)
    }

    fn remove_ended(&mut self) {
        let events = &mut self.events;
        self.active.retain(|track| {
            let ended = track.source_done && track.backlog.is_empty() && track.wait == 0;
            if ended {
                lock(&track.control).state = PlayState::Ended;
                events.push(TrackEvent::Ended {
                    track: track.id,
                    reason: track.end_reason(),
                });
            }
            !ended
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioError;
    use crate::audio::ogg::OggOpusSource;
    use crate::audio::pcm::PcmSource;

    /// A source that gives scripted frames, then an optional error.
    struct Script {
        frames: VecDeque<SourceFrame>,
        fail: bool,
    }

    impl Script {
        fn opus(packets: &[&[u8]]) -> Self {
            let frames = packets
                .iter()
                .map(|packet| SourceFrame::opus(packet.to_vec()))
                .collect();
            Self {
                frames,
                fail: false,
            }
        }
    }

    impl AudioSource for Script {
        fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
            if let Some(frame) = self.frames.pop_front() {
                return Ok(Some(frame));
            }
            if self.fail {
                return Err(AudioError::Format("broken".to_owned()));
            }
            Ok(None)
        }
    }

    fn constant(value: f32, frames: usize) -> PcmSource {
        PcmSource::from_f32(vec![value; FRAME_LEN * frames])
    }

    fn pcm(output: &MixOutput) -> &[f32] {
        match output {
            MixOutput::Pcm(samples) => samples,
            other => panic!("PCM expected, got {other:?}"),
        }
    }

    #[test]
    fn opus_passes_through() {
        let mut mixer = Mixer::new();
        let track = mixer.play(Script::opus(&[&[0xFC, 1], &[0x18, 2], &[0xFC, 3]]));
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0xFC, 1],
                samples: 960
            }
        );
        // A 60 ms packet covers three steps.
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0x18, 2],
                samples: 2880
            }
        );
        assert_eq!(mixer.next_frame(), MixOutput::Busy);
        assert_eq!(mixer.next_frame(), MixOutput::Busy);
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0xFC, 3],
                samples: 960
            }
        );
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.state(), PlayState::Ended);
        assert_eq!(
            mixer.take_events(),
            vec![
                TrackEvent::Started(track.id()),
                TrackEvent::Ended {
                    track: track.id(),
                    reason: EndReason::Finished
                }
            ]
        );
        assert!(mixer.is_empty());
    }

    #[test]
    fn pcm_tracks_are_added_with_volume() {
        let mut mixer = Mixer::new();
        let first = mixer.play(constant(0.25, 2));
        let second = mixer.play(Track::new(constant(0.5, 1)).volume(0.5));
        let output = mixer.next_frame();
        assert!(
            pcm(&output)
                .iter()
                .all(|sample| (sample - 0.5).abs() < 1e-6)
        );

        let output = mixer.next_frame();
        assert!(
            pcm(&output)
                .iter()
                .all(|sample| (sample - 0.25).abs() < 1e-6)
        );
        assert_eq!(second.state(), PlayState::Ended);

        first.set_volume(4.0);
        assert!((first.volume() - 4.0).abs() < f32::EPSILON);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(first.state(), PlayState::Ended);
    }

    #[test]
    fn clipping() {
        let mut mixer = Mixer::new();
        mixer.play(constant(0.75, 1));
        mixer.play(constant(0.75, 1));
        let output = mixer.next_frame();
        assert!(
            pcm(&output)
                .iter()
                .all(|sample| (sample - 1.0).abs() < 1e-6)
        );
    }

    #[test]
    fn pause_resume_stop() {
        let mut mixer = Mixer::new();
        let track = mixer.play(constant(0.5, 3));
        track.pause();
        assert_eq!(track.state(), PlayState::Paused);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        track.resume();
        assert!(matches!(mixer.next_frame(), MixOutput::Pcm(_)));
        track.stop();
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.state(), PlayState::Ended);
        let events = mixer.take_events();
        assert_eq!(
            events.last(),
            Some(&TrackEvent::Ended {
                track: track.id(),
                reason: EndReason::Stopped
            })
        );

        let paused = mixer.play(Track::new(constant(0.5, 1)).paused());
        assert_eq!(paused.state(), PlayState::Paused);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
    }

    #[test]
    fn queue_plays_in_order() {
        let mut mixer = Mixer::new();
        let first = mixer.enqueue(Script::opus(&[&[0xFC, 1]]));
        let second = mixer.enqueue(Script::opus(&[&[0xFC, 2], &[0xFC, 3]]));
        let third = mixer.enqueue(Script::opus(&[&[0xFC, 4]]));
        assert_eq!(mixer.queue_len(), 3);

        let packets: Vec<MixOutput> = (0..3).map(|_| mixer.next_frame()).collect();
        assert_eq!(
            packets,
            vec![
                MixOutput::Opus {
                    packet: vec![0xFC, 1],
                    samples: 960
                },
                MixOutput::Opus {
                    packet: vec![0xFC, 2],
                    samples: 960
                },
                MixOutput::Opus {
                    packet: vec![0xFC, 3],
                    samples: 960
                },
            ]
        );
        assert_eq!(first.state(), PlayState::Ended);
        assert_eq!(second.state(), PlayState::Playing);

        mixer.skip();
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0xFC, 4],
                samples: 960
            }
        );
        assert_eq!(second.state(), PlayState::Ended);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(third.state(), PlayState::Ended);
        assert_eq!(mixer.queue_len(), 0);

        let ended: Vec<(TrackId, EndReason)> = mixer
            .take_events()
            .into_iter()
            .filter_map(|event| match event {
                TrackEvent::Ended { track, reason } => Some((track, reason)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ended,
            vec![
                (first.id(), EndReason::Finished),
                (second.id(), EndReason::Stopped),
                (third.id(), EndReason::Finished),
            ]
        );
    }

    #[test]
    fn stop_all_and_errors() {
        let mut mixer = Mixer::new();
        let failing = mixer.play(Script {
            frames: VecDeque::new(),
            fail: true,
        });
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert!(mixer.take_events().contains(&TrackEvent::Ended {
            track: failing.id(),
            reason: EndReason::Error("the audio input is not valid: broken".to_owned())
        }));

        let playing = mixer.play(constant(0.1, 10));
        let waiting = mixer.enqueue(constant(0.1, 10));
        let later = mixer.enqueue(constant(0.1, 10));
        mixer.stop_all();
        assert!(mixer.is_empty());
        for track in [playing, waiting, later] {
            assert_eq!(track.state(), PlayState::Ended);
        }
    }

    #[test]
    fn seek_and_position() {
        // Each frame holds its index divided by 10.
        let samples: Vec<f32> = (0..FRAME_LEN * 5)
            .map(|index| f32::from(u16::try_from(index / FRAME_LEN).expect("small")) / 10.0)
            .collect();
        let mut mixer = Mixer::new();
        let track = mixer.play(PcmSource::from_f32(samples));
        assert!(pcm(&mixer.next_frame())[0].abs() < 1e-6);
        assert_eq!(track.position(), Duration::from_millis(20));

        track.seek(Duration::from_millis(60));
        assert!((pcm(&mixer.next_frame())[0] - 0.3).abs() < 1e-6);
        assert_eq!(track.position(), Duration::from_millis(80));
        assert!(mixer.take_events().contains(&TrackEvent::Seeked {
            track: track.id(),
            position: Duration::from_millis(60),
        }));

        let scripted = mixer.play(Script::opus(&[&[0xFC, 1]]));
        scripted.seek(Duration::from_secs(1));
        mixer.next_frame();
        assert!(mixer.take_events().iter().any(|event| matches!(
            event,
            TrackEvent::SeekFailed { track, .. } if *track == scripted.id()
        )));
    }

    #[test]
    fn ogg_seek_passes_through() {
        let packets: Vec<Vec<u8>> = (0..10).map(|index| vec![0xFC, index]).collect();
        let file = crate::audio::ogg::tests::build_file(0, &packets, 4, None);
        let mut mixer = Mixer::new();
        let track = mixer.play(OggOpusSource::from_bytes(file).expect("valid"));
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0xFC, 0],
                samples: 960
            }
        );

        // 130 ms is in packet 6 (120 ms to 140 ms). The whole packet goes out.
        track.seek(Duration::from_millis(130));
        assert_eq!(
            mixer.next_frame(),
            MixOutput::Opus {
                packet: vec![0xFC, 6],
                samples: 960
            }
        );
        assert_eq!(track.position(), Duration::from_millis(140));
    }

    #[test]
    fn waiting_source_keeps_its_track() {
        let frames = VecDeque::from([
            SourceFrame::Waiting,
            SourceFrame::Pcm(vec![0.5; FRAME_LEN]),
            SourceFrame::Waiting,
            SourceFrame::Pcm(vec![0.25; FRAME_LEN]),
        ]);
        let mut mixer = Mixer::new();
        let track = mixer.play(Script {
            frames,
            fail: false,
        });
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.state(), PlayState::Playing);
        assert!((pcm(&mixer.next_frame())[0] - 0.5).abs() < 1e-6);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert!((pcm(&mixer.next_frame())[0] - 0.25).abs() < 1e-6);
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.state(), PlayState::Ended);

        // In a mix, the other track plays while one track waits.
        let frames = VecDeque::from([SourceFrame::Waiting, SourceFrame::Pcm(vec![0.5; FRAME_LEN])]);
        mixer.play(Script {
            frames,
            fail: false,
        });
        mixer.play(constant(0.25, 2));
        assert!((pcm(&mixer.next_frame())[0] - 0.25).abs() < 1e-6);
        assert!((pcm(&mixer.next_frame())[0] - 0.75).abs() < 1e-6);
    }

    #[cfg(not(feature = "opus"))]
    #[test]
    fn short_packets_need_opus() {
        let mut mixer = Mixer::new();
        // CELT packets of 10 ms.
        let track = mixer.play(Script::opus(&[&[0xF0, 1], &[0xF0, 2]]));
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.state(), PlayState::Ended);
        assert!(mixer.take_events().iter().any(|event| matches!(
            event,
            TrackEvent::Ended {
                reason: EndReason::Error(_),
                ..
            }
        )));
    }

    #[cfg(feature = "opus")]
    #[test]
    fn short_packets_are_encoded_again() {
        let mut encoder =
            opus::Encoder::new(48_000, opus::Channels::Stereo, opus::Application::Audio)
                .expect("encoder");
        let packet = encoder
            .encode_vec_float(&vec![0.0; FRAME_LEN / 2], 1275)
            .expect("encode");
        assert_eq!(crate::audio::opus_packet_samples(&packet), Some(480));

        let mut mixer = Mixer::new();
        let track = mixer.play(Script::opus(&[packet.as_slice(); 4]));
        assert!(matches!(mixer.next_frame(), MixOutput::Pcm(_)));
        assert!(matches!(mixer.next_frame(), MixOutput::Pcm(_)));
        assert_eq!(mixer.next_frame(), MixOutput::Idle);
        assert_eq!(track.position(), Duration::from_millis(40));
        assert_eq!(track.state(), PlayState::Ended);
    }

    #[cfg(feature = "opus")]
    #[test]
    fn pre_skip_and_end_trim_are_decoded() {
        let mut encoder = crate::audio::codec::OpusEncoder::new().expect("encoder");
        let packets: Vec<Vec<u8>> = (0..5)
            .map(|_| encoder.encode(&vec![0.0; FRAME_LEN]).expect("encode"))
            .collect();
        let file = crate::audio::ogg::tests::build_file(312, &packets, 2, Some(4800 - 200));

        let mut mixer = Mixer::new();
        let source = OggOpusSource::from_bytes(file).expect("valid");
        // A volume change makes the mixer decode the packets.
        let track = mixer.play(Track::new(source).volume(0.5));
        let mut frames = 0;
        while let MixOutput::Pcm(_) = mixer.next_frame() {
            frames += 1;
        }
        // 4800 - 312 - 200 = 4288 samples: four full frames and one part of a frame.
        assert_eq!(frames, 5);
        assert_eq!(track.position(), samples_to_duration(4288));
        assert_eq!(track.state(), PlayState::Ended);
    }

    #[cfg(feature = "opus")]
    #[test]
    fn opus_is_decoded_to_mix() {
        let mut encoder = crate::audio::codec::OpusEncoder::new().expect("encoder");
        let packet = encoder.encode(&vec![0.0; FRAME_LEN]).expect("encode");
        let mut mixer = Mixer::new();
        let track = mixer.play(Script::opus(&[&packet, &packet]));
        track.set_volume(0.5);
        assert!(matches!(mixer.next_frame(), MixOutput::Pcm(_)));
        mixer.play(constant(0.25, 1));
        track.set_volume(1.0);
        let output = mixer.next_frame();
        assert!(
            pcm(&output)
                .iter()
                .all(|sample| (sample - 0.25).abs() < 0.05)
        );
    }
}
