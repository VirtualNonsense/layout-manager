use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
/// - Filtering is controlled by the `RUST_LOG` env var (falls back to `info` if unset).
/// - Logs are written to daily-rotating files in `~/.local/<CRATE_NAME>/logs`.
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
        .with_env_filter(env_filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    Ok(LoggingGuard {
        _worker_guard: worker_guard,
    })
}

/// Returns the last `n` lines across all rotated log files for this crate,
/// newest-first — both across files (newest file first) and within each
/// file (last line of the file first).
///
/// This is fully lazy: files are opened and read one chunk at a time only
/// as the returned iterator is advanced. If the first file (or two)
/// already contains `n` lines, later/older log files are never opened at
/// all.
pub fn tail_log_lines(n: usize) -> io::Result<impl Iterator<Item = String>> {
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

    // Chain a lazy tail-iterator for each file, newest file first, then
    // cap the combined stream at `n` lines total.
    let iter = log_files
        .into_iter()
        .flat_map(|(_, path)| {
            // `TailLines::new` returns `io::Result<TailLines>`. `Result`
            // implements `IntoIterator`, yielding the `Ok` value once or
            // nothing on `Err` — so a file that fails to open (e.g. it
            // was removed between the directory scan and now) simply
            // contributes an empty sub-iterator instead of aborting the
            // whole call, matching the previous `continue`-on-error
            // behavior.
            //
            // `.flatten()` then unwraps that outer Result-as-iterator
            // and lets `TailLines`'s own `Iterator` impl take over,
            // giving a flat `Iterator<Item = io::Result<String>>`.
            TailLines::new(&path, usize::MAX)
                .into_iter()
                .flatten()
                // Drop individual line/read errors rather than
                // propagating them — same "skip on error" spirit as the
                // original per-file `match ... Err(_) => continue`.
                .filter_map(Result::ok)
        })
        .take(n);

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
///
/// Because it's a real [`Iterator`], you can combine it with adaptors like
/// [`Iterator::take`] to read only as much of the file as needed:
///
/// ```no_run
/// # use std::path::Path;
/// # fn tail_lines_iter(_: &Path, _: usize) -> std::io::Result<TailLines> { unimplemented!() }
/// # struct TailLines;
/// # impl Iterator for TailLines { type Item = std::io::Result<String>; fn next(&mut self) -> Option<Self::Item> { None } }
/// # fn main() -> std::io::Result<()> {
/// let path = Path::new("app.log");
///
/// // Print the last 20 lines, newest first.
/// for line in tail_lines_iter(path, 20)? {
///     println!("{}", line?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TailLines {
    /// The open file handle we're reading backwards from.
    file: File,
    /// Byte offset in the file up to which we've already consumed data.
    /// The next chunk read (if any) will end exactly at this offset.
    pos: u64,
    /// Bytes read from the file so far but not yet turned into a line.
    /// Ordered the same as in the file (front = earliest byte in buffer).
    buffer: VecDeque<u8>,
    /// True once we've read all the way back to the start of the file,
    /// meaning there is no more data to fetch from disk.
    reached_start: bool,
    /// Number of lines still to be yielded before the iterator stops.
    /// Use `usize::MAX` for "no limit" (read until start of file).
    remaining: usize,
}

impl TailLines {
    /// Opens `path` and prepares to yield up to `n` lines from the end of
    /// the file, newest-first.
    ///
    /// No data is read from disk until [`next`](Iterator::next) is first
    /// called — construction only opens the file and seeks to determine
    /// its length.
    ///
    /// Pass `n = usize::MAX` to iterate over the entire file backwards
    /// without a fixed limit (useful in combination with `.take(k)` at the
    /// call site).
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
    ///
    /// Returns `Ok(true)` if a chunk was read, `Ok(false)` if we were
    /// already at the start of the file (nothing left to read).
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

        // Prepend the chunk's bytes to the front of the buffer, preserving
        // their original order.
        for &byte in chunk.iter().rev() {
            self.buffer.push_front(byte);
        }

        if self.pos == 0 {
            self.reached_start = true;
        }

        Ok(true)
    }

    /// Pops the trailing complete line out of `self.buffer`, if the buffer
    /// currently ends in one (i.e. contains at least one `\n`).
    ///
    /// The trailing newline itself is discarded and is not part of the
    /// returned string. Returns `None` if the buffer holds no newline yet.
    fn take_trailing_line(&mut self) -> Option<String> {
        let newline_idx = self.buffer.iter().rposition(|&b| b == b'\n')?;

        // Split off everything after the newline: that's our line's bytes.
        let line_bytes: Vec<u8> = self.buffer.split_off(newline_idx + 1).into();
        // Discard the newline itself, now the last element of `buffer`.
        self.buffer.pop_back();

        Some(String::from_utf8_lossy(&line_bytes).into_owned())
    }
}

impl Iterator for TailLines {
    /// Each item is a line of text, or an I/O error if reading the file
    /// failed. Errors are non-fatal for the type itself but by convention
    /// you should stop iterating once you see one.
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        loop {
            // Fast path: buffer already ends with a complete line.
            if let Some(line) = self.take_trailing_line() {
                self.remaining -= 1;
                return Some(Ok(line));
            }

            // No newline found yet. If we've read the whole file, whatever
            // remains in the buffer is the final (first-in-file) line,
            // which may not have a trailing newline (e.g. no EOF newline).
            if self.reached_start {
                if self.buffer.is_empty() {
                    return None;
                }
                let line_bytes: Vec<u8> = std::mem::take(&mut self.buffer).into();
                self.remaining -= 1;
                return Some(Ok(String::from_utf8_lossy(&line_bytes).into_owned()));
            }

            // Otherwise, fetch more data and try again.
            if let Err(e) = self.fill_chunk() {
                return Some(Err(e));
            }
        }
    }
}
