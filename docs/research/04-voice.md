# Research: Voice

Research date: 2026-11-05.

## Sources

- `developers/topics/voice-connections.mdx`, `developers/topics/opcodes-and-status-codes.mdx`, change log.
- DAVE whitepaper v1.1.4: <https://github.com/discord/dave-protocol/blob/main/protocol.md>, <https://daveprotocol.com>.
- libdave: <https://github.com/discord/libdave>.
- Discord blog, 2026-05-18: every call except Stage is E2EE.
- **[UNOFFICIAL]** marks community sources (docs.discord.food, discord.js source).

## 1. Connection

1. Send main gateway op 4 `{guild_id, channel_id, self_mute, self_deaf}`.
2. Wait for both events:
   - `VOICE_STATE_UPDATE` (has `session_id`).
   - `VOICE_SERVER_UPDATE` `{token, guild_id, endpoint?}`.
3. Connect to `wss://{endpoint}/?v=8`.

Rules:

- Do not cache these values.
- A null `endpoint` means the server is being reallocated. Disconnect and wait for the next update.
- A channel move in the same guild changes the token. The old session is not valid.
- A full channel sends no events, unless the bot has MOVE_MEMBERS.

| Voice gateway version | Status | Change |
|---|---|---|
| 8 | Recommended | Server message buffering, `seq_ack` |
| 7 | Available | Channel options opcode |
| 6 | Available | Code version opcode |
| 5 | Available | Video sink wants |
| 4 | Available | Speaking is a bitmask |
| 1..3 | Rejected since 2024-11-18 | |

Flow:

1. Receive op 8 Hello `{heartbeat_interval}` (float ms).
2. Send op 0 Identify `{server_id, user_id, session_id, token, max_dave_protocol_version}`.
3. Receive op 2 Ready `{ssrc, ip, port, modes}`. Ignore its `heartbeat_interval`.
4. Open UDP to `ip:port`. Do IP discovery.
5. Send op 1 Select Protocol `{protocol: "udp", data: {address, port, mode}}`.
6. Receive op 4 Session Description `{mode, secret_key: [32 u8], dave_protocol_version}`.
7. Send op 5 Speaking before the first audio packet.

Heartbeat (v8): send op 3 `{t: nonce, seq_ack}`. ACK is op 6 `{t: nonce}`.

Resume: new socket, op 7 `{server_id, session_id, token, seq_ack}`. Success gives op 9 Resumed.

Buffered resume (v8):

- JSON messages carry `seq`. Binary messages start with a 2-byte BE seq.
- Send `seq_ack: -1` if nothing arrived yet.
- After resume, the server sends missed messages again.

| Op | Name | Direction | Binary |
|---|---|---|---|
| 0 | Identify | client | |
| 1 | Select Protocol | client | |
| 2 | Ready | server | |
| 3 | Heartbeat | client | |
| 4 | Session Description | server | |
| 5 | Speaking | both | |
| 6 | Heartbeat ACK | server | |
| 7 | Resume | client | |
| 8 | Hello | server | |
| 9 | Resumed | server | |
| 11 | Clients Connect `{user_ids}` | server | |
| 13 | Client Disconnect `{user_id}` | server | |
| 21 | DAVE Prepare Transition | server | |
| 22 | DAVE Execute Transition | server | |
| 23 | DAVE Transition Ready | client | |
| 24 | DAVE Prepare Epoch | server | |
| 25 | DAVE MLS External Sender | server | yes |
| 26 | DAVE MLS Key Package | client | yes |
| 27 | DAVE MLS Proposals | server | yes |
| 28 | DAVE MLS Commit Welcome | client | yes |
| 29 | DAVE MLS Announce Commit Transition | server | yes |
| 30 | DAVE MLS Welcome | server | yes |
| 31 | DAVE MLS Invalid Commit Welcome | client | no |

- [UNOFFICIAL] Other opcodes occur (12, 14, 15, 16, 18, 20, 32). Ignore unknown opcodes.

| Close code | Meaning |
|---|---|
| 4001 | Unknown opcode |
| 4002 | Decode failure |
| 4003 | Not authenticated |
| 4004 | Authentication failed |
| 4005 | Already authenticated |
| 4006 | Session not valid |
| 4009 | Session timeout |
| 4011 | Server not found |
| 4012 | Unknown protocol |
| 4014 | Disconnected (kicked). Do not reconnect. |
| 4015 | Voice server crashed. Resume. |
| 4016 | Unknown encryption mode |
| 4017 | DAVE (E2EE) required |
| 4020 | Bad request |
| 4021 | Rate limited. Do not reconnect. |
| 4022 | Call terminated. Do not reconnect. |

## 2. UDP

IP discovery (big-endian, 74 bytes): type u16 (1 request, 2 response), length u16 (70), SSRC u32, address (64 bytes, null-terminated), port u16.

RTP header: `0x80`, `0x78` (PT 120), sequence u16 BE, timestamp u32 BE, SSRC u32 BE.

- X bit `0x10` in byte 0. CC = low 4 bits.
- [UNOFFICIAL] Extensions use profile `0xBEDE`. In rtpsize modes the 4-byte preamble is AAD. The extension elements are encrypted. Skip `4 * length` bytes after decryption.
- Strip RTP padding after decryption.

Opus: 48 kHz, 2 channels, 20 ms frames (960 samples per frame).

- Send 5 silence frames (`F8 FF FE`) before a pause.
- Speaking (op 5): `{speaking, delay: 0, ssrc}`. Flags: Microphone 1<<0, Soundshare 1<<1, Priority 1<<2.
- [UNOFFICIAL] UDP keepalive packets with magic `0x1337CAFE` / `0x1337F00D`. Filter them.

## 3. Transport encryption

- Supported: `aead_aes256_gcm_rtpsize` (preferred when offered) and `aead_xchacha20_poly1305_rtpsize` (always offered; required).
- Removed 2024-11-18: all `xsalsa20_poly1305*` modes and `aead_aes256_gcm`.
- Packet: `[header + CSRCs + ext preamble] [ciphertext] [16-byte tag] [4-byte nonce]`.
- AAD = header (12 + 4*CC + 4 if X).
- Nonce: 32-bit counter, big-endian in the first 4 bytes of a zeroed 12-byte (AES) or 24-byte (XChaCha) nonce. Appended raw at the end of the packet. (Layout from discord.js source.)
- Key: 32-byte `secret_key`.
- Transport encryption wraps the DAVE frame.
- [UNOFFICIAL] RTCP (`byte[1]` in 200..=206) arrives on the same socket. Drop it.

## 4. DAVE (end-to-end encryption)

Status:

- DAVE protocol version 1.
- Non-DAVE connections close with 4017 since 2026-03-02.
- Stage channels are not E2EE.
- **A bot library must implement DAVE v1 for voice in guild voice channels, DMs, and group DMs.**

MLS parameters:

- MLS 1.0 (RFC 9420). Ciphersuite 2: `MLS_128_DHKEMP256_AES128GCM_SHA256_P256`.
- Basic credentials. Identity = user snowflake as big-endian u64.
- One group extension: `external_senders` (from op 25).
- Key package lifetime: `not_before = 0`, `not_after = 2^64 - 1`.
- Handshake messages are PublicMessage. GREASE disabled.
- The Welcome includes an inline ratchet tree.

Binary frame: `[u16 BE seq (server only)] [u8 opcode] [payload]`. TLS presentation language with MLS varint vectors.

| Op | Payload |
|---|---|
| 25 | ExternalSender `{signature_key, credential{type=1, identity}}` |
| 26 | MLSMessage(KeyPackage) |
| 27 | u8 operation (0 append, 1 revoke), then proposals or proposal refs |
| 28 | MLSMessage commit, optional Welcome |
| 29 | u16 transition_id, MLSMessage commit |
| 30 | u16 transition_id, Welcome |

JSON ops: 21 `{protocol_version, transition_id}`, 22 `{transition_id}`, 23 `{transition_id}`, 24 `{protocol_version, epoch}`, 31 `{transition_id}`.

Flows:

- On Session Description with version > 0, on op 24 with `epoch = 1`, or after op 31: reset the group, create a one-member group, send a new key package (op 26).
- Op 27: accept only Add/Remove from the external sender. Refuse Adds for users not in op 11. Commit and send op 28.
- Op 29: apply the winning commit, build per-sender decryptors, send op 23.
- Op 30: join through the Welcome. Check the external sender. Send op 23.
- Op 22: switch the send side to the new keys or version.
- Op 21 to version 0: passthrough mode. `transition_id == 0` executes immediately.
- Invalid commit or welcome: send op 31, reset, send a new op 26.
- Keep old-epoch keys for up to 10 s.

Keys:

- `base = MLS-Exporter("Discord Secure Frames v0", little-endian u64 sender user ID, 16)`.
- Hash ratchet (RFC 9420 §9.1 style). `key(gen)` = 16-byte AES-128 key.
- Generation = high byte of the 32-bit truncated nonce. Nonce resets each epoch.
- libdave limits: max generation gap 250, max missing nonces 1000, expiry 10 s.

Frame format (Opus frame, before transport encryption):

`[ciphertext] [8-byte truncated AES-128-GCM tag] [ULEB128 nonce] [ULEB128 unencrypted ranges] [u8 supplemental size] [0xFAFA]`

- Supplemental size = tag + nonce + ranges + 1 + 2.
- GCM nonce = 8 zero bytes + 32-bit truncated nonce (little-endian in libdave; check with test vectors).
- No AAD for Opus. The whole frame is encrypted.
- Reject replays.
- The frame `F8 FF FE` passes through unencrypted.

Implementations:

- libdave: C++17, MIT, C API (`dave.h`). Needs mlspp and OpenSSL or BoringSSL.
- `davey` (Rust, MIT, 0.2.0): OpenMLS + RustCrypto. Used by discord.js and discord.py through bindings.
- `dave` 0.3.0 (Rust): AGPL-3.0. Do not use.
- OpenMLS 0.9.2 supports ciphersuite 2 with `openmls_rust_crypto`.

## 5. Receive audio

- Op 5 from the server: `{user_id, ssrc, speaking}`. This maps SSRC to user.
- Op 11: users who joined (no SSRC). Needed for DAVE Add checks.
- Op 13: remove that user's SSRC and decryptor.
- Packets can arrive before op 5. Buffer them for a short time or drop them.

Packet pipeline:

1. Drop discovery, keepalive, and RTCP packets.
2. Check version 2 and PT 120.
3. Transport-decrypt with header AAD.
4. Strip padding and extension body.
5. DAVE-decrypt with the decryptor for the user of that SSRC.
6. Opus-decode.

- A jitter buffer per SSRC is necessary: reorder by sequence, use Opus PLC/FEC for gaps.

## 6. Related resources

- `GET /voice/regions`. Channel `rtc_region` (null = automatic), `video_quality_mode`.
- Voice state REST: GET and PATCH `/guilds/{g}/voice-states/@me` and `/{user_id}` (Stage suppress and request to speak).
- Stage instances: CRUD. `privacy_level` GUILD_ONLY 2.
- Voice channel status: `PUT /channels/{c}/voice-status` `{status (max 500)}`. Read through gateway op 43.
- Soundboard: `POST /channels/{c}/send-soundboard-sound` `{sound_id, source_guild_id?}`. The bot must be connected to that channel. It does not need to send RTP.
- Video and Go Live: not available to bots. Out of scope.

## Open points

- Whether JSON and binary messages share one seq counter (very likely).
- Byte order of the DAVE truncated nonce in the GCM nonce.
- Whether `dave_protocol_version` is 0 in Stage channels.
- RTP extension IDs, RTCP, keepalive, and extra opcodes are community knowledge only.
