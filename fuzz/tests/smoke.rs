//! A stable smoke test of the fuzz bodies.
//!
//! libFuzzer needs nightly. This test runs each fuzz body on stable over a fixed set of
//! pseudo-random inputs: random bytes, and mutations of the docs fixtures and of hand-made
//! seeds. The inputs are the same in each run, so a failure repeats.
//!
//! Set `CORDIAL_FUZZ_ITERATIONS` to change the number of inputs for each target.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::{Path, PathBuf};

use cordial_fuzz::seeds;

/// The default number of generated inputs for each target.
const DEFAULT_ITERATIONS: usize = 20_000;

/// The largest random input.
const MAX_RANDOM_LEN: usize = 512;

/// Text that often changes the path of a JSON decoder.
const TOKENS: [&[u8]; 16] = [
    b"{",
    b"}",
    b"[",
    b"]",
    b"\"",
    b",",
    b":",
    b"null",
    b"true",
    b"-1",
    b"0",
    b"1e309",
    b"18446744073709551616",
    b"\"18446744073709551615\"",
    b"\n",
    b"\\u0000",
];

/// A xorshift64* generator. It needs no security, only the same output in each run.
struct Rng(u64);

impl Rng {
    fn new(seed: &str) -> Self {
        let hash = seed.bytes().fold(0xCBF2_9CE4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
        });
        Self(hash | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Gives a number below `bound`. `bound` must not be zero.
    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("a small bound");
        usize::try_from(self.next() % bound).expect("a value below the bound")
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }
}

fn iterations() -> usize {
    std::env::var("CORDIAL_FUZZ_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/docs")
}

/// Reads all JSON files under a directory.
fn read_json_files(dir: &Path, found: &mut Vec<Vec<u8>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            read_json_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Ok(bytes) = std::fs::read(&path)
        {
            found.push(bytes);
        }
    }
}

/// Gives the seeds of a target. Each seed starts with a selector byte.
fn target_seeds(name: &str) -> Vec<Vec<u8>> {
    let mut fixtures = Vec::new();
    read_json_files(&fixtures_dir(), &mut fixtures);
    assert!(!fixtures.is_empty(), "the docs fixtures are missing");

    // The fixtures are JSON bodies; each selector value can use them.
    let mut found: Vec<Vec<u8>> = fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            let selector = u8::try_from(index % 256).expect("below 256");
            [&[selector], fixture.as_slice()].concat()
        })
        .collect();
    found.extend(seeds::for_target(name));
    found
}

fn mutate(rng: &mut Rng, input: &mut Vec<u8>, seeds: &[Vec<u8>]) {
    let count = 1 + rng.below(8);
    for _ in 0..count {
        let len = input.len();
        match rng.below(8) {
            0 if len > 0 => {
                let index = rng.below(len);
                input[index] ^= 1 << rng.below(8);
            }
            1 if len > 0 => {
                let index = rng.below(len);
                input[index] = rng.byte();
            }
            2 => {
                let index = rng.below(len + 1);
                input.insert(index, rng.byte());
            }
            3 if len > 0 => {
                let start = rng.below(len);
                let end = (start + 1 + rng.below(16)).min(len);
                input.drain(start..end);
            }
            4 if len > 0 => {
                input.truncate(rng.below(len));
            }
            5 if len > 0 => {
                let start = rng.below(len);
                let end = (start + 1 + rng.below(64)).min(len);
                let copy = input[start..end].to_vec();
                let index = rng.below(len + 1);
                input.splice(index..index, copy);
            }
            6 => {
                let token = TOKENS[rng.below(TOKENS.len())];
                let index = rng.below(len + 1);
                input.splice(index..index, token.iter().copied());
            }
            _ => {
                // Splice: the start of this input and the end of another seed.
                let other = &seeds[rng.below(seeds.len())];
                let cut = rng.below(len + 1);
                let other_cut = rng.below(other.len() + 1);
                input.truncate(cut);
                input.extend_from_slice(&other[other_cut..]);
            }
        }
    }
}

fn generate(rng: &mut Rng, seeds: &[Vec<u8>]) -> Vec<u8> {
    if rng.below(4) == 0 {
        let len = rng.below(MAX_RANDOM_LEN + 1);
        return (0..len).map(|_| rng.byte()).collect();
    }
    let mut input = seeds[rng.below(seeds.len())].clone();
    mutate(rng, &mut input, seeds);
    input
}

fn run_once(name: &str, body: fn(&[u8]), input: &[u8]) {
    if let Err(panic) = catch_unwind(AssertUnwindSafe(|| body(input))) {
        let hex: String = input.iter().map(|byte| format!("{byte:02x}")).collect();
        eprintln!("target `{name}` panics on the input (hex):\n{hex}");
        resume_unwind(panic);
    }
}

fn run(name: &str) {
    let body = cordial_fuzz::TARGETS
        .iter()
        .find(|(target, _)| *target == name)
        .map(|(_, body)| *body)
        .expect("a known target");
    let seeds = target_seeds(name);

    for seed in &seeds {
        run_once(name, body, seed);
    }
    let mut rng = Rng::new(name);
    for _ in 0..iterations() {
        let input = generate(&mut rng, &seeds);
        run_once(name, body, &input);
    }
}

#[test]
fn gateway_payload() {
    run("gateway_payload");
}

#[test]
fn gateway_compression() {
    run("gateway_compression");
}

#[test]
fn http_response() {
    run("http_response");
}

#[test]
fn voice_packet() {
    run("voice_packet");
}

#[test]
fn voice_gateway() {
    run("voice_gateway");
}

#[test]
fn dave_frame() {
    run("dave_frame");
}

#[test]
fn ogg() {
    run("ogg");
}

#[test]
fn endpoint() {
    run("endpoint");
}

#[test]
fn command_parse() {
    run("command_parse");
}

#[test]
fn each_target_has_a_test() {
    assert_eq!(cordial_fuzz::TARGETS.len(), 9);
}

/// Writes the seeds of each target to `fuzz/corpus/<target>`, the default corpus directory of
/// `cargo fuzz`. Run it once before the first `cargo fuzz run`.
#[test]
#[ignore = "writes files; run it by hand"]
fn write_corpus() {
    for (name, _) in cordial_fuzz::TARGETS {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("corpus")
            .join(name);
        std::fs::create_dir_all(&dir).expect("cannot make the corpus directory");
        for (index, seed) in target_seeds(name).iter().enumerate() {
            std::fs::write(dir.join(format!("seed-{index:04}")), seed)
                .expect("cannot write a seed");
        }
    }
}
