use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Holds resources that must stay alive for the duration of the program.
/// If this is dropped, the non-blocking writer will stop flushing logs.
pub struct LoggingGuard {
    _worker_guard: WorkerGuard,
}

/// Returns the log directory: ~/.local/<CRATE_NAME>/logs
fn log_dir() -> PathBuf {
    std::env::home_dir()
        .expect("could not determine home directory")
        .join(".local")
        .join(CRATE_NAME)
        .join("logs")
}

/// Initializes the global tracing subscriber.
///
/// - Filtering is controlled by the `RUST_LOG` env var (falls back to `trace` if unset).
/// - Logs are written to daily-rotating files in `~/.local/<CRATE_NAME>/logs`.
/// - Output is **line-delimited JSON**: one self-describing object per line,
///   so it can be parsed losslessly via [`tail_log_entries`].
///
/// Returns a guard that must be kept alive for the lifetime of the program.
pub fn init_logging() -> io::Result<LoggingGuard> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(format!("{CRATE_NAME}.log"))
        .max_log_files(14)
        .build(&dir)
        .expect("failed to initialize rolling file appender");

    let (non_blocking, worker_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter =
        EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("trace"));

    tracing_subscriber::fmt()
        .json()
        // Keep the active span chain, but as structured data — not folded
        // into the message. We intentionally do NOT enable `with_span_events`,
        // so every emitted line is a real event that maps 1:1 to a `LogEntry`.
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
    /// A fixed-width, uppercase label for the level (nice for column alignment).
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A single parsed log event.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// When the event was recorded.
    pub timestamp: DateTime<Utc>,
    /// Severity level.
    pub level: LogLevel,
    /// The human-readable message text.
    pub message: String,
    /// Names of the spans that were active when the event fired,
    /// outermost first. Empty if the event fired outside any span.
    pub spans: Vec<String>,
}

/// Mirrors the shape tracing-subscriber's JSON formatter emits. Only the
/// fields we care about are pulled out; everything else is ignored.
#[derive(Deserialize)]
struct RawLogLine {
    timestamp: String,
    level: LogLevel,
    #[serde(default)]
    fields: RawFields,
    /// The list of active spans, outermost first. Present when
    /// `with_span_list(true)` is set.
    #[serde(default)]
    spans: Vec<RawSpan>,
}

#[derive(Deserialize, Default)]
struct RawFields {
    /// tracing puts the log message under `fields.message`.
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct RawSpan {
    #[serde(default)]
    name: String,
    // Any span fields (key=value pairs) are captured by serde but ignored.
}

impl LogEntry {
    /// Parse a single JSON log line into a [`LogEntry`].
    ///
    /// Returns `None` if the line isn't valid JSON in the expected shape
    /// (e.g. a blank line, a partially-written tail, or output from a
    /// different formatter).
    fn parse(line: &str) -> Option<LogEntry> {
        let raw: RawLogLine = serde_json::from_str(line).ok()?;
        Some(LogEntry {
            timestamp: DateTime::parse_from_rfc3339(&raw.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()?,
            // skip lines with an unparseable timestamp,
            level: raw.level,
            message: raw.fields.message,
            spans: raw.spans.into_iter().map(|s| s.name).collect(),
        })
    }
}

/// Returns the last `n` log entries across all rotated log files for this
/// crate, newest-first — both across files (newest file first) and within
/// each file (last line first).
///
/// This is fully lazy: files are opened and read one chunk at a time only
/// as the returned iterator is advanced. Lines that fail to parse (blank
/// lines, torn writes, etc.) are silently skipped and do **not** count
/// against `n`, so you always get up to `n` real entries.
pub fn tail_log_entries(n: usize) -> io::Result<impl Iterator<Item = LogEntry>> {
    Ok(tail_log_lines_raw()?
        .filter_map(|line| LogEntry::parse(&line))
        .take(n))
}

/// Shared, uncapped backward line iterator over all log files newest-first.
fn tail_log_lines_raw() -> io::Result<impl Iterator<Item = String>> {
    let dir = log_dir();
    let prefix = format!("{CRATE_NAME}.log.");

    // Collect just the file *paths* sorted by mtime descending. We don't
    // read any file contents here — that happens lazily below.
    let mut log_files: Vec<(SystemTime, PathBuf)> = fs::read_dir(&dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            if !name.starts_with(&prefix) {
                return None;
            }
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((mtime, path))
        })
        .collect();

    log_files.sort_by_key(|b| std::cmp::Reverse(b.0));

    // Chain a lazy tail-iterator for each file, newest file first.
    let iter = log_files.into_iter().flat_map(|(_, path)| {
        TailLines::new(&path, usize::MAX)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
    });

    Ok(iter)
}

/// Size of each backward read from the file, in bytes.
const CHUNK_SIZE: u64 = 8192;

/// A lazy iterator over the lines of a file, read from the end backwards.
///
/// Yields lines in **newest-first** order (i.e. the last line of the file
/// comes first). Reads the file in fixed-size chunks, scanning backwards,
/// so it never needs to load the whole file into memory — only enough of
/// the tail to satisfy the lines actually consumed.
pub struct TailLines {
    /// The open file handle we're reading backwards from.
    file: File,
    /// Byte offset in the file up to which we've already consumed data.
    /// The next chunk read (if any) will end exactly at this offset.
    pos: u64,
    /// Bytes read from the file so far but not yet turned into a line.
    /// Ordered the same as in the file (front = earliest byte in buffer).
    buffer: VecDeque<u8>,
    /// True once we've read all the way back to the start of the file.
    reached_start: bool,
    /// Number of lines still to be yielded before the iterator stops.
    /// Use `usize::MAX` for "no limit" (read until start of file).
    remaining: usize,
}

impl TailLines {
    /// Opens `path` and prepares to yield up to `n` lines from the end of
    /// the file, newest-first. No data is read until `next` is first called.
    pub fn new(path: &Path, n: usize) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let pos = file.seek(SeekFrom::End(0))?;
        Ok(Self {
            file,
            pos,
            buffer: VecDeque::new(),
            reached_start: pos == 0,
            remaining: n,
        })
    }

    /// Reads one more chunk of the file, immediately preceding the bytes
    /// we've already buffered, and prepends it to `self.buffer`.
    fn fill_chunk(&mut self) -> io::Result<bool> {
        if self.pos == 0 {
            self.reached_start = true;
            return Ok(false);
        }

        let read_size = CHUNK_SIZE.min(self.pos);
        self.pos -= read_size;

        self.file.seek(SeekFrom::Start(self.pos))?;
        let mut chunk = vec![0u8; read_size as usize];
        self.file.read_exact(&mut chunk)?;

        for &byte in chunk.iter().rev() {
            self.buffer.push_front(byte);
        }

        if self.pos == 0 {
            self.reached_start = true;
        }

        Ok(true)
    }

    /// Pops the trailing complete line out of `self.buffer`, if the buffer
    /// currently ends in one. The trailing newline is discarded.
    fn take_trailing_line(&mut self) -> Option<String> {
        let newline_idx = self.buffer.iter().rposition(|&b| b == b'\n')?;
        let line_bytes: Vec<u8> = self.buffer.split_off(newline_idx + 1).into();
        self.buffer.pop_back();
        Some(String::from_utf8_lossy(&line_bytes).into_owned())
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
                return Some(Ok(String::from_utf8_lossy(&line_bytes).into_owned()));
            }

            if let Err(e) = self.fill_chunk() {
                return Some(Err(e));
            }
        }
    }
}
