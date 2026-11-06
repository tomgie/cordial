//! Tests of `ProcessSource` with a real child process.
//!
//! The test binary starts itself as the external program. An environment variable tells
//! the child which role to play. This test has no libtest harness (`harness = false` in
//! `Cargo.toml`): a harness writes its own text to the standard output, and the child must
//! write only PCM bytes there. A shell command is not used, because shells on Windows and
//! Unix do not write binary data in the same way.

use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant};

use cordial_voice::audio::FRAME_LEN;
use cordial_voice::audio::pcm::i16_to_f32;
use cordial_voice::{AudioError, AudioSource, ProcessSource, SourceFrame};

/// The environment variable that holds the role of the child process.
const ROLE: &str = "CORDIAL_TEST_PROCESS_ROLE";
/// The number of samples that the `pcm` role writes: two and a half frames.
const SAMPLE_COUNT: usize = FRAME_LEN * 5 / 2;
/// The longest time that a test waits for the child.
const TIMEOUT: Duration = Duration::from_secs(30);

fn main() {
    match std::env::var(ROLE).as_deref() {
        Ok("pcm") => write_pcm(),
        Ok("fail") => fail(),
        Ok("sleep") => sleep(),
        _ => {
            let tests: [(&str, fn()); 4] = [
                ("reads_pcm", reads_pcm),
                ("reports_a_failure", reports_a_failure),
                ("missing_program", missing_program),
                ("drop_kills_the_child", drop_kills_the_child),
            ];
            let mut stderr = std::io::stderr();
            for (name, test) in tests {
                test();
                let _ = writeln!(stderr, "test {name} ... ok");
            }
        }
    }
}

fn known_sample(index: usize) -> i16 {
    i16::try_from(index * 13 % 2000).expect("small") - 1000
}

fn write_pcm() {
    let bytes: Vec<u8> = (0..SAMPLE_COUNT)
        .flat_map(|index| known_sample(index).to_le_bytes())
        .collect();
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes).expect("write");
    stdout.flush().expect("flush");
}

fn fail() {
    let mut stderr = std::io::stderr().lock();
    stderr
        .write_all(b"first line\ncannot open the input file\n")
        .expect("write");
    std::process::exit(3);
}

fn sleep() {
    let silence = vec![0_u8; FRAME_LEN * 2];
    std::io::stdout().lock().write_all(&silence).expect("write");
    std::io::stdout().lock().flush().expect("flush");
    std::thread::sleep(Duration::from_secs(120));
}

fn child(role: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("test binary"));
    command.env(ROLE, role);
    command
}

/// Reads frames until the end. Waits while the source has no audio yet.
fn collect(source: &mut ProcessSource) -> Result<Vec<Vec<f32>>, AudioError> {
    let start = Instant::now();
    let mut frames = Vec::new();
    loop {
        assert!(start.elapsed() < TIMEOUT, "the child is too slow");
        match source.next_frame()? {
            Some(SourceFrame::Pcm(frame)) => frames.push(frame),
            Some(SourceFrame::Waiting) => std::thread::sleep(Duration::from_millis(2)),
            Some(other) => panic!("PCM expected, got {other:?}"),
            None => return Ok(frames),
        }
    }
}

fn reads_pcm() {
    let mut source = ProcessSource::command(child("pcm")).expect("start");
    let frames = collect(&mut source).expect("no error");
    assert_eq!(frames.len(), 3);
    assert!(frames.iter().all(|frame| frame.len() == FRAME_LEN));

    let samples: Vec<f32> = frames.concat();
    for (index, sample) in samples.iter().enumerate() {
        let expected = if index < SAMPLE_COUNT {
            i16_to_f32(known_sample(index))
        } else {
            0.0
        };
        assert!(
            (sample - expected).abs() < f32::EPSILON,
            "sample {index}: {sample} is not {expected}"
        );
    }
    assert_eq!(source.next_frame().expect("no error"), None);
    assert!(matches!(
        source.seek(Duration::ZERO),
        Err(AudioError::Unsupported(_))
    ));
}

fn reports_a_failure() {
    let mut source = ProcessSource::command(child("fail")).expect("start");
    let error = collect(&mut source).expect_err("the child fails");
    let AudioError::Process(message) = error else {
        panic!("a process error expected, got {error:?}");
    };
    assert!(message.contains("cannot open the input file"), "{message}");
}

fn missing_program() {
    let error = ProcessSource::command(Command::new("cordial-test-no-such-program"))
        .expect_err("the program does not exist");
    assert!(matches!(error, AudioError::Process(_)), "{error:?}");
}

/// The drop kills the child and waits for its exit. The child sleeps for 120 seconds, so a
/// fast drop shows that the kill worked: the wait returns only after the child stops.
fn drop_kills_the_child() {
    let mut source = ProcessSource::command(child("sleep")).expect("start");
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < TIMEOUT, "the child is too slow");
        match source.next_frame().expect("no error") {
            Some(SourceFrame::Pcm(_)) => break,
            Some(SourceFrame::Waiting) => std::thread::sleep(Duration::from_millis(2)),
            other => panic!("PCM expected, got {other:?}"),
        }
    }

    let start = Instant::now();
    drop(source);
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "the drop did not stop the child"
    );
}
