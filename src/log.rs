use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Map, Value};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Holds resources that must stay alive for the duration of the program.
///
/// If this is dropped, the non-blocking writer will stop flushing logs.
pub struct LoggingGuard {
    _worker_guard: WorkerGuard,
}

/// Returns the log directory:
///
/// `~/.local/<CRATE_NAME>/logs`
fn log_dir() -> PathBuf {
    std::env::home_dir()
        .expect("could not determine home directory")
        .join(".local")
        .join(CRATE_NAME)
        .join("logs")
}

/// Initializes the global tracing subscriber.
///
/// - Filtering is controlled by the `RUST_LOG` environment variable.
/// - The default filter is `trace`.
/// - Logs are written to daily rotating files.
/// - At most 14 log files are retained.
/// - Every log event is written as line-delimited JSON.
///
/// The returned guard must be kept alive for the lifetime of the program.
pub fn init_logging() -> io::Result<LoggingGuard> {
    let dir = log_dir();
    fs::create_dir_all(&dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(format!("{CRATE_NAME}.log"))
        .max_log_files(14)
        .build(&dir)
        .map_err(io::Error::other)?;

    let (non_blocking, worker_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter =
        EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::fmt()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_env_filter(env_filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    Ok(LoggingGuard {
        _worker_guard: worker_guard,
    })
}

/// Severity of a log event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Returns a fixed-width uppercase label.
    pub const fn label(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO ",
            LogLevel::Warn => "WARN ",
            LogLevel::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// A tracing span that was active when an event was emitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSpan {
    /// Name of the span.
    pub name: String,

    /// Structured fields attached to the span.
    pub fields: Map<String, Value>,
}

/// A single parsed tracing log event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    /// When the event was recorded.
    pub timestamp: DateTime<Utc>,

    /// Severity level of the event.
    pub level: LogLevel,

    /// Human-readable message.
    pub message: String,

    /// Structured event fields excluding `message`.
    pub fields: Map<String, Value>,

    /// Tracing target, usually the Rust module path.
    pub target: String,

    /// Active spans, ordered outermost first.
    pub spans: Vec<LogSpan>,
}

/// Mirrors the relevant portions of tracing-subscriber's JSON output.
#[derive(Deserialize)]
struct RawLogLine {
    timestamp: String,
    level: LogLevel,

    #[serde(default)]
    fields: Map<String, Value>,

    #[serde(default)]
    target: String,

    #[serde(default)]
    spans: Vec<RawSpan>,
}

/// Raw representation of one active tracing span.
///
/// Any properties other than `name` are captured as structured span fields.
#[derive(Deserialize)]
struct RawSpan {
    #[serde(default)]
    name: String,

    #[serde(flatten)]
    fields: Map<String, Value>,
}

impl LogEntry {
    /// Parses one JSON log line.
    ///
    /// Returns `None` when the input is not valid JSON in the expected
    /// tracing-subscriber format. This includes blank lines and partially
    /// written events.
    fn parse(line: &str) -> Option<Self> {
        let mut raw: RawLogLine = serde_json::from_str(line).ok()?;

        let timestamp = DateTime::parse_from_rfc3339(&raw.timestamp)
            .map(|timestamp| timestamp.with_timezone(&Utc))
            .ok()?;

        let message = raw
            .fields
            .remove("message")
            .map(value_to_message)
            .unwrap_or_default();

        let spans = raw
            .spans
            .into_iter()
            .map(|span| LogSpan {
                name: span.name,
                fields: span.fields,
            })
            .collect();

        Some(Self {
            timestamp,
            level: raw.level,
            message,
            fields: raw.fields,
            target: raw.target,
            spans,
        })
    }
}

/// Converts a JSON value from the `message` field into readable text.
///
/// tracing normally serializes the message as a string, but handling other
/// JSON value types here makes the parser more tolerant.
fn value_to_message(value: Value) -> String {
    match value {
        Value::String(message) => message,
        other => other.to_string(),
    }
}

/// Returns the last `n` valid log entries across all rotated log files.
///
/// Entries are returned newest first:
///
/// - Newer files are processed before older files.
/// - Lines within each file are processed from bottom to top.
///
/// Invalid JSON lines and partially written lines are skipped and do not
/// count toward `n`.
pub fn tail_log_entries(n: usize) -> io::Result<impl Iterator<Item = LogEntry>> {
    Ok(tail_log_lines_raw()?
        .filter_map(|line| LogEntry::parse(&line))
        .take(n))
}

/// Returns an uncapped backward line iterator over all log files.
fn tail_log_lines_raw() -> io::Result<impl Iterator<Item = String>> {
    let dir = log_dir();

    // Treat an application without any logs as an empty log directory.
    fs::create_dir_all(&dir)?;

    let prefix = format!("{CRATE_NAME}.log.");

    let mut log_files: Vec<(SystemTime, PathBuf)> = fs::read_dir(&dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            let name = path.file_name()?.to_string_lossy();

            if !name.starts_with(&prefix) {
                return None;
            }

            let modified = entry.metadata().ok()?.modified().ok()?;

            Some((modified, path))
        })
        .collect();

    log_files.sort_by_key(|entry| std::cmp::Reverse(entry.0));

    let iterator = log_files.into_iter().flat_map(|(_, path)| {
        TailLines::new(&path, usize::MAX)
            .into_iter()
            .flatten()
            .filter_map(|result| match result {
                Ok(line) => Some(line),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "failed to read a line from log file",
                    );
                    None
                }
            })
    });

    Ok(iterator)
}

/// Size of each backward file read, in bytes.
const CHUNK_SIZE: u64 = 8192;

/// A lazy iterator over the lines of a file, reading from end to start.
///
/// Lines are yielded newest first. The complete file is never loaded into
/// memory. Only fixed-size pieces of the file and the current partial line
/// are retained.
pub struct TailLines {
    /// Open file being read.
    file: File,

    /// Byte offset at which the next backward read ends.
    pos: u64,

    /// Data read so far that has not yet been returned as a complete line.
    buffer: VecDeque<u8>,

    /// Whether the beginning of the file has been reached.
    reached_start: bool,

    /// Maximum number of lines that can still be returned.
    remaining: usize,

    /// Whether a trailing newline at the end of the file was already ignored.
    ignored_trailing_newline: bool,
}

impl TailLines {
    /// Opens a file and prepares to return up to `n` lines from its end.
    ///
    /// No file contents are read until `next` is called.
    pub fn new(path: &Path, n: usize) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let pos = file.seek(SeekFrom::End(0))?;

        Ok(Self {
            file,
            pos,
            buffer: VecDeque::new(),
            reached_start: pos == 0,
            remaining: n,
            ignored_trailing_newline: false,
        })
    }

    /// Reads the chunk immediately preceding the current buffer.
    fn fill_chunk(&mut self) -> io::Result<bool> {
        if self.pos == 0 {
            self.reached_start = true;
            return Ok(false);
        }

        let read_size = CHUNK_SIZE.min(self.pos);
        self.pos -= read_size;

        self.file.seek(SeekFrom::Start(self.pos))?;

        let mut chunk = vec![0_u8; read_size as usize];
        self.file.read_exact(&mut chunk)?;

        for byte in chunk.into_iter().rev() {
            self.buffer.push_front(byte);
        }

        if self.pos == 0 {
            self.reached_start = true;
        }

        Ok(true)
    }

    /// Removes and returns the last complete line in the buffer.
    fn take_trailing_line(&mut self) -> Option<String> {
        let newline_index = self.buffer.iter().rposition(|byte| *byte == b'\n')?;

        let line_bytes: Vec<u8> = self.buffer.split_off(newline_index + 1).into();

        // Remove the newline that remains at the end of the original buffer.
        self.buffer.pop_back();

        // A normal text file commonly ends in '\n'. That delimiter does not
        // represent an additional empty log entry.
        if line_bytes.is_empty() && !self.ignored_trailing_newline {
            self.ignored_trailing_newline = true;
            return self.take_trailing_line();
        }

        Some(decode_line(line_bytes))
    }
}

impl Iterator for TailLines {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        loop {
            if let Some(line) = self.take_trailing_line() {
                self.remaining -= 1;
                return Some(Ok(line));
            }

            if self.reached_start {
                if self.buffer.is_empty() {
                    return None;
                }

                let line_bytes: Vec<u8> = std::mem::take(&mut self.buffer).into();

                self.remaining -= 1;

                return Some(Ok(decode_line(line_bytes)));
            }

            if let Err(error) = self.fill_chunk() {
                return Some(Err(error));
            }
        }
    }
}

/// Decodes a log line and removes a possible carriage return.
///
/// Removing `\r` supports files using `\r\n` line endings.
fn decode_line(mut bytes: Vec<u8>) -> String {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }

    String::from_utf8_lossy(&bytes).into_owned()
}
