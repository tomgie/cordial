# Fuzz targets

This directory holds the fuzz targets of cordial. It is a separate package with its own workspace. Normal builds of the main workspace do not compile it.

## Layout

| Path | Content |
|---|---|
| `src/` | The fuzz bodies: one `pub fn` for each target. libFuzzer and the smoke test call the same functions. |
| `src/seeds.rs` | Hand-made seed inputs that reach deep paths. |
| `fuzz_targets/` | One libFuzzer binary for each target. They need the `libfuzzer` feature. |
| `tests/smoke.rs` | A stable test: it runs each body over 20,000 fixed pseudo-random inputs. |

## Targets

| Target | Decoders |
|---|---|
| `gateway_payload` | Gateway frames, `Event::decode` for each event name, the shard core, and the cache. |
| `gateway_compression` | The `zlib-stream` and `zstd-stream` decompressors, with arbitrary chunks and a round trip. |
| `http_response` | The JSON error body, the rate-limit headers and the rate limiter, and `Message`, `Guild`, `Interaction`, and `Channel`. |
| `voice_packet` | Packet kinds, RTP headers, IP discovery, transport decryption, and the receive pipeline. |
| `voice_gateway` | Text and binary frames of the voice gateway, with and without a DAVE session. |
| `dave_frame` | The DAVE frame footer, decryption with a fixed key, and the DAVE opcode payloads of a session. |
| `ogg` | The Ogg page and packet readers and `OggOpusSource`. |
| `endpoint` | The interaction endpoint with a fixed key (signed and unsigned requests), and the body decode. |
| `command_parse` | Command option parsing (`OptionsInput`, a derived command) and `custom_id` patterns. |

Two library crates have an internal `fuzzing` feature for the targets: `cordial-gateway` (the decompressor) and `cordial-voice` (the DAVE frame codec). It is not part of the public API.

## Run the smoke test (stable)

```sh
cargo test --manifest-path fuzz/Cargo.toml
```

Set `CORDIAL_FUZZ_ITERATIONS` to change the number of inputs for each target.

## Run libFuzzer (nightly)

1. Install the tools: `rustup toolchain install nightly` and `cargo install cargo-fuzz`.
2. Write the seeds once: `cargo test --manifest-path fuzz/Cargo.toml -- --ignored write_corpus`.
3. Run a target: `cargo +nightly fuzz run --features libfuzzer <target> -- -max_total_time=60`.

On Windows with MSVC, the address sanitizer needs `clang_rt.asan_dynamic-x86_64.dll`. Put the Visual Studio folder `VC\Tools\MSVC\<version>\bin\Hostx64\x64` on `PATH` before the run.

A crash writes the input to `fuzz/artifacts/<target>/`. To repeat it, give the file to the target: `cargo +nightly fuzz run --features libfuzzer <target> <file>`.
