//! Fuzz bodies for the decoders of cordial.
//!
//! Each public function takes one fuzz input. The libFuzzer targets in `fuzz_targets/` and the
//! stable smoke test in `tests/smoke.rs` call the same functions. A body must not panic for
//! any input. A panic is a bug in the library, except for a failed `assert!` here, which is a
//! broken invariant (also a bug).
//!
//! The first input byte of most bodies is a selector. It chooses a mode or a name.

mod command;
mod dave;
mod endpoint;
mod gateway;
mod http;
mod ogg;
mod reader;
pub mod seeds;
mod voice;

pub use command::command_parse;
pub use dave::dave_frame;
pub use endpoint::endpoint;
pub use gateway::{gateway_compression, gateway_payload};
pub use http::http_response;
pub use ogg::ogg;
pub use voice::{voice_gateway, voice_packet};

/// The body of a fuzz target: it takes one input.
pub type Body = fn(&[u8]);

/// The names and the bodies of all fuzz targets.
pub const TARGETS: &[(&str, Body)] = &[
    ("gateway_payload", gateway_payload),
    ("gateway_compression", gateway_compression),
    ("http_response", http_response),
    ("voice_packet", voice_packet),
    ("voice_gateway", voice_gateway),
    ("dave_frame", dave_frame),
    ("ogg", ogg),
    ("endpoint", endpoint),
    ("command_parse", command_parse),
];

thread_local! {
    // Paused time: sleeps end at once, so that a long rate-limit wait does not block a body.
    static RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .expect("cannot make the Tokio runtime");
}

/// Runs a future to completion on the runtime of this thread.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    RUNTIME.with(|runtime| runtime.block_on(future))
}
