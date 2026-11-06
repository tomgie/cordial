#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| cordial_fuzz::http_response(data));
