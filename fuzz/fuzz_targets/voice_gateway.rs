#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| cordial_fuzz::voice_gateway(data));
