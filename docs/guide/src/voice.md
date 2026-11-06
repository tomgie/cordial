# Voice

The crate `cordial-voice` (module `cordial::voice`) connects a bot to voice channels. It
sends and receives audio. Turn on the `voice` feature of `cordial`.

## Features and CMake

| Feature of `cordial-voice` | Gives | Needs |
|---|---|---|
| (none) | Ogg Opus files, receive of Opus packets | |
| `opus` | PCM sources, process sources, decode of received audio | CMake and a C compiler (libopus) |
| `symphonia` | MP3, FLAC, WAV, Vorbis, and AAC files | turns on `opus` |

The facade crate does not forward these features. Add `cordial-voice` with the same
version to turn them on:

```toml
[dependencies]
cordial = { version = "0.7", features = ["voice"] }
cordial-voice = { version = "0.7", features = ["opus"] }
```

Without `opus`, the bot does not send PCM audio and logs one warning. Ogg Opus files need
no encoder: the bot sends their packets as they are. Only Ogg Opus packets that are not
20 ms long need `opus`.

## DAVE: end-to-end encryption

Discord encrypts the audio in voice channels end to end with the DAVE protocol. DAVE is
mandatory. A call without DAVE ends with close code 4017.

`DaveSession` implements DAVE. Give the voice manager a factory that makes one session for
each call:

```rust
VoiceManager::with_e2ee(shards, CallConfig::default(), || {
    Box::new(DaveSession::new().expect("the DAVE key pair is made")) as Box<dyn E2eeSession>
})
```

`VoiceManager::new` uses no end-to-end encryption. Use it only for tests.

## The voice manager

`VoiceManager` joins and leaves voice channels. It keeps one `Call` for each guild. It
needs these gateway events:

| Event | Use |
|---|---|
| `READY` | The user ID of the bot. `set_user_id` also sets it. |
| `VOICE_STATE_UPDATE` | The voice session of the bot. Needs the `GUILD_VOICE_STATES` intent. |
| `VOICE_SERVER_UPDATE` | The voice server and its token. |

Give each event to `VoiceManager::handle_event`. The manager ignores the other events.

The manager sends Update Voice State through the main gateway. It needs the shard handles,
and they exist only after the bot starts. Thus the example makes the manager in the
`READY` handler:

```rust
{{#include ../../../crates/cordial/tests/guide/voice.rs:manager}}
```

A `Vec<ShardHandle>` routes each guild to its shard. It computes the shard from the
length of the list, so give it all shards of the bot.

## Join and play

`join` sends the voice state, waits for the two voice events, and connects. It gives the
`Call` when audio can flow. A join can take more than 3 seconds, so defer the interaction
first.

```rust
{{#include ../../../crates/cordial/tests/guide/voice.rs:play}}
```

`leave` leaves the channel and stops the call. `Call::disconnect` stops only the
connection: the bot stays in the channel for Discord.

## Sources

A source gives 20 ms frames of audio. Discord audio is Opus at 48 kHz, stereo.

| Source | Input | Seek | Needs |
|---|---|---|---|
| `OggOpusSource::open(path)` | An Ogg Opus file | yes | |
| `OggOpusSource::new(reader)` | An Ogg Opus stream | no | |
| `OggOpusSource::seekable(reader)` | An Ogg Opus stream with `Seek` | yes | |
| `PcmSource::from_f32(samples)`, `from_i16(samples)` | Interleaved stereo PCM at 48 kHz in memory | yes | `opus` |
| `PcmSource::from_reader(reader)` | Signed 16-bit little-endian PCM | no | `opus` |
| `ProcessSource::ffmpeg(input)` | A file or a URL, decoded by ffmpeg | no | `opus`, ffmpeg |
| `ProcessSource::command(command)` | The PCM output of a program | no | `opus` |
| `audio::decode::DecodedSource::open(path)` | MP3, FLAC, WAV, Vorbis, AAC | yes | `symphonia` |

A process source cannot seek. Start the program at the position instead, for example with
the `-ss` option of ffmpeg: change `ProcessSource::ffmpeg_command` and give it to
`ProcessSource::command`.

Implement the `AudioSource` trait for other inputs.

## The mixer and tracks

Each call has a mixer. A track is a source with its settings.

| `Call` method | Effect |
|---|---|
| `play(track)` | Starts the track now. It plays at the same time as other tracks. |
| `enqueue(track)` | Adds the track to the end of the queue. |
| `skip()` | Stops the current track of the queue. The next track starts. |
| `stop_all()` | Stops all tracks and clears the queue. |
| `queue_len()` | The number of tracks in the queue. |

`play` and `enqueue` give a `TrackHandle`:

| `TrackHandle` method | Effect |
|---|---|
| `pause()`, `resume()` | Pauses and resumes the track. |
| `stop()` | Stops the track. |
| `set_volume(volume)` | 1.0 is the original volume, 0.0 is silent. |
| `seek(position)` | Moves the track to a position. |
| `position()` | The position of the track. |
| `state()` | The play state. |

`Track::new(source).volume(0.5).paused()` sets the start settings.

## Seek

`TrackHandle::seek` asks the mixer to move the track. The mixer applies it before the next
frame and sends an event:

- `TrackEvent::Seeked { position, .. }`: the track plays from `position`. It can be a
  little before the requested position.
- `TrackEvent::SeekFailed { error, .. }`: the source cannot seek. The track plays on.

## Events

`Call::subscribe` gives a receiver of `VoiceEvent` values: the state of the connection,
speaking users, users that join or leave, track events, and received audio. A slow receiver
misses old events.

## Receive audio

Turn on receive with `CallConfig::default().receive(true)`. Each 20 ms of audio of each
user comes as `VoiceEvent::Audio(VoiceData)`:

| Field | Value |
|---|---|
| `user_id` | The user of the audio, when the voice gateway told it. |
| `opus` | The Opus packet after all decryption. `None` for a lost packet. |
| `pcm` | Decoded stereo PCM at 48 kHz. Needs the `opus` feature and `decode(true)`. |

A jitter buffer puts the packets in order. For a lost packet, the decoder makes a
replacement frame (packet loss concealment).

## Other call methods

| Method | Use |
|---|---|
| `send_soundboard_sound` | Plays a soundboard sound in the channel. |
| `request_to_speak`, `set_suppressed` | Stage channels. |
| `set_channel_status` | Sets the status text of the voice channel. |
| `wait_ready(timeout)` | Waits until the voice session is ready. |
| `latency()` | The heartbeat latency of the voice gateway. |
