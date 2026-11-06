//! A source that reads PCM from an external program, for example ffmpeg.
//!
//! The program writes signed 16-bit little-endian stereo samples at 48 kHz to its standard
//! output. Two threads read the standard output and the standard error of the program, so
//! [`AudioSource::next_frame`] never blocks the Tokio runtime.

use std::ffi::OsStr;
use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use super::pcm::read_pcm_frame;
use super::{AudioError, AudioSource, SourceFrame};

/// The number of frames that the reader thread keeps ready: 2 seconds.
const BUFFER_FRAMES: usize = 100;
/// The time to wait for the exit status after the program closes its output.
const EXIT_TIMEOUT: Duration = Duration::from_secs(1);
/// The time between two checks of the exit status.
const EXIT_POLL: Duration = Duration::from_millis(10);

/// A message from the reader thread.
enum Message {
    Frame(Vec<f32>),
    Error(AudioError),
    /// The output ended. The exit status is `None` when the program still runs.
    End(Option<ExitStatus>),
}

type SharedChild = Arc<Mutex<Child>>;
type LastLine = Arc<Mutex<Option<String>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An audio source that runs an external program and reads PCM from its standard output.
///
/// The program must write signed 16-bit little-endian stereo samples at 48 kHz. Use
/// [`ProcessSource::ffmpeg`] to decode a file or a URL with ffmpeg. Use
/// [`ProcessSource::command`] for another program or a pipeline.
///
/// - The source needs the `opus` feature to be sent, because the send task encodes PCM.
/// - The source cannot seek. Start the program at a position instead, for example with the
///   `-ss` option of ffmpeg.
/// - When the program is slower than real time, the track is silent until audio arrives.
/// - Each line on the standard error of the program goes to the log at the debug level.
///
/// When the source is dropped, it kills the program and waits for its exit. The kill
/// reaches only the program that the source starts. When this program is a shell that
/// starts other programs, those programs stop when they write to the closed pipe.
pub struct ProcessSource {
    child: SharedChild,
    program: String,
    messages: Receiver<Message>,
    last_error_line: LastLine,
    finished: bool,
}

impl fmt::Debug for ProcessSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessSource")
            .field("program", &self.program)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl ProcessSource {
    /// Starts ffmpeg to decode an input: a file path or a URL that ffmpeg can read.
    ///
    /// The command is the one that [`ProcessSource::ffmpeg_command`] gives.
    ///
    /// # Errors
    ///
    /// Gives [`AudioError::Process`] when ffmpeg is not installed or cannot start.
    pub fn ffmpeg(input: impl AsRef<OsStr>) -> Result<Self, AudioError> {
        Self::command(Self::ffmpeg_command(input))
    }

    /// Gives the ffmpeg command that [`ProcessSource::ffmpeg`] runs. Change it and give it
    /// to [`ProcessSource::command`], for example to add a start position.
    ///
    /// The command is `ffmpeg -nostdin -loglevel error -i <input> -vn -f s16le -ar 48000
    /// -ac 2 pipe:1`.
    #[must_use]
    pub fn ffmpeg_command(input: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new("ffmpeg");
        command
            .args(["-nostdin", "-loglevel", "error", "-i"])
            .arg(input)
            .args(["-vn", "-f", "s16le", "-ar", "48000", "-ac", "2", "pipe:1"]);
        command
    }

    /// Starts a program that writes signed 16-bit little-endian stereo PCM at 48 kHz to its
    /// standard output.
    ///
    /// The source sets the standard input of the program to null and reads its standard
    /// output and standard error.
    ///
    /// # Errors
    ///
    /// Gives [`AudioError::Process`] when the program is not found or cannot start.
    pub fn command(mut command: Command) -> Result<Self, AudioError> {
        let program = command.get_program().to_string_lossy().into_owned();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| spawn_error(&program, &error))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(child));
        let (Some(stdout), Some(stderr)) = (stdout, stderr) else {
            stop(&child);
            return Err(AudioError::Process(format!(
                "`{program}` has no output pipe"
            )));
        };

        match start_threads(&program, Arc::clone(&child), stdout, stderr) {
            Ok((messages, last_error_line)) => Ok(Self {
                child,
                program,
                messages,
                last_error_line,
                finished: false,
            }),
            Err(error) => {
                stop(&child);
                Err(AudioError::Process(format!(
                    "cannot start a reader thread for `{program}`: {error}"
                )))
            }
        }
    }

    /// Gives the operating system ID of the program.
    #[must_use]
    pub fn id(&self) -> u32 {
        lock(&self.child).id()
    }

    fn end_result(&self, status: Option<ExitStatus>) -> Result<Option<SourceFrame>, AudioError> {
        let Some(status) = status.filter(|status| !status.success()) else {
            return Ok(None);
        };
        let detail = lock(&self.last_error_line)
            .clone()
            .map_or_else(String::new, |line| format!(": {line}"));
        Err(AudioError::Process(format!(
            "`{}` stops with {status}{detail}",
            self.program
        )))
    }
}

impl AudioSource for ProcessSource {
    fn next_frame(&mut self) -> Result<Option<SourceFrame>, AudioError> {
        if self.finished {
            return Ok(None);
        }

        match self.messages.try_recv() {
            Ok(Message::Frame(frame)) => Ok(Some(SourceFrame::Pcm(frame))),
            Err(TryRecvError::Empty) => Ok(Some(SourceFrame::Waiting)),
            Ok(Message::Error(error)) => {
                self.finished = true;
                Err(error)
            }
            Ok(Message::End(status)) => {
                self.finished = true;
                self.end_result(status)
            }
            Err(TryRecvError::Disconnected) => {
                self.finished = true;
                Ok(None)
            }
        }
    }
}

impl Drop for ProcessSource {
    fn drop(&mut self) {
        stop(&self.child);
    }
}

/// Kills the program and reaps it, so that no zombie process stays.
fn stop(child: &Mutex<Child>) {
    let mut child = lock(child);
    // The kill fails when the program has already stopped. The wait reaps it in both cases.
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_error(program: &str, error: &std::io::Error) -> AudioError {
    if error.kind() == ErrorKind::NotFound {
        AudioError::Process(format!(
            "the program `{program}` is not found; install it or add it to PATH"
        ))
    } else {
        AudioError::Process(format!("cannot start `{program}`: {error}"))
    }
}

fn start_threads(
    program: &str,
    child: SharedChild,
    stdout: ChildStdout,
    stderr: ChildStderr,
) -> std::io::Result<(Receiver<Message>, LastLine)> {
    let last_error_line = LastLine::default();
    let (sender, messages) = mpsc::sync_channel(BUFFER_FRAMES);
    let (stderr_done, stderr_finished) = mpsc::channel::<()>();

    let line_store = Arc::clone(&last_error_line);
    let name = program.to_owned();
    thread::Builder::new()
        .name("cordial-process-stderr".to_owned())
        .spawn(move || {
            log_stderr(&name, stderr, &line_store);
            drop(stderr_done);
        })?;

    thread::Builder::new()
        .name("cordial-process-stdout".to_owned())
        .spawn(move || read_stdout(stdout, &child, &sender, &stderr_finished))?;
    Ok((messages, last_error_line))
}

/// Reads frames until the output ends, then reports the exit status.
fn read_stdout(
    mut stdout: ChildStdout,
    child: &Mutex<Child>,
    sender: &SyncSender<Message>,
    stderr_finished: &Receiver<()>,
) {
    loop {
        match read_pcm_frame(&mut stdout) {
            // A send error means that the source is dropped.
            Ok(Some(frame)) => {
                if sender.send(Message::Frame(frame)).is_err() {
                    return;
                }
            }
            Ok(None) => break,
            Err(error) => {
                let _ = sender.send(Message::Error(error));
                return;
            }
        }
    }

    // The error message of the program must be in the log before the end is reported.
    let _ = stderr_finished.recv_timeout(EXIT_TIMEOUT);
    let status = wait_for_exit(child);
    let _ = sender.send(Message::End(status));
}

/// Waits a short time for the exit status. A program can close its output and run on.
fn wait_for_exit(child: &Mutex<Child>) -> Option<ExitStatus> {
    let attempts = EXIT_TIMEOUT.as_millis() / EXIT_POLL.as_millis();
    for _ in 0..attempts {
        match lock(child).try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        thread::sleep(EXIT_POLL);
    }
    None
}

fn log_stderr(program: &str, stderr: ChildStderr, last_error_line: &Mutex<Option<String>>) {
    let mut reader = BufReader::new(stderr);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&bytes).trim_end().to_owned();
        if line.is_empty() {
            continue;
        }
        tracing::debug!(program, "{line}");
        *lock(last_error_line) = Some(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_arguments() {
        let command = ProcessSource::ffmpeg_command("song.mp3");
        assert_eq!(command.get_program(), "ffmpeg");
        let arguments: Vec<&OsStr> = command.get_args().collect();
        let expected = [
            "-nostdin",
            "-loglevel",
            "error",
            "-i",
            "song.mp3",
            "-vn",
            "-f",
            "s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "pipe:1",
        ];
        assert_eq!(arguments, expected.map(OsStr::new));
    }

    #[test]
    fn missing_program() {
        let error = ProcessSource::command(Command::new("cordial-test-no-such-program"))
            .expect_err("the program does not exist");
        let AudioError::Process(message) = error else {
            panic!("a process error expected, got {error:?}");
        };
        assert!(message.contains("not found"), "{message}");
    }
}
