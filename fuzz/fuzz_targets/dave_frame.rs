#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| cordial_fuzz::dave_frame(data));
