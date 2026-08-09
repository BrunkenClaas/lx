#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::exit::LxError;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default maximum stdin bytes read before truncation (512 KiB).
pub const DEFAULT_MAX_INPUT_BYTES: usize = 512 * 1024;

/// Aggregate ceiling for tools that read many files into memory at once.
///
/// `max_input_bytes` is a **per-source** limit. A tool that walks a directory
/// multiplies it by the file count, so memory is unbounded without a second
/// ceiling: at 512 KiB across 1000 files that is already 500 MB. Tools that
/// walk directories must budget against this in addition to the per-file limit.
pub const DEFAULT_MAX_TOTAL_INPUT_BYTES: usize = 64 * 1024 * 1024;

// ── Byte-safe truncation ──────────────────────────────────────────────────────

/// Truncate `s` to at most `max_bytes`, snapping back to the nearest character
/// boundary so the result is always valid UTF-8 and never ends mid-character.
///
/// Slicing a `&str` at a raw byte offset panics when that offset falls inside a
/// multi-byte character, which is what any tool doing `&input[..MAX]` risks on
/// non-ASCII input. Use this instead of a bare range slice for every input cap.
pub fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // `is_char_boundary` is O(1) and a UTF-8 character is at most 4 bytes, so
    // this steps back at most three times.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Drop a trailing incomplete UTF-8 sequence from `buf`.
///
/// Only meaningful after a truncating read: cutting a byte stream at a fixed
/// offset can split a multi-byte character, and `from_utf8_lossy` would turn
/// that fragment into a replacement character the user never wrote. Callers
/// must NOT use this on a complete read — there, invalid bytes are the user's
/// real data and should stay marked as such.
fn trim_incomplete_utf8_tail(buf: &[u8]) -> &[u8] {
    let n = buf.len();
    // A UTF-8 character is at most 4 bytes, so only the last 3 can be partial.
    for back in 1..=3.min(n) {
        let i = n - back;
        let b = buf[i];
        if b < 0x80 {
            break; // ASCII byte: the tail is complete.
        }
        if b >= 0xC0 {
            // Lead byte: keep it only if all its continuation bytes arrived.
            let need = if b >= 0xF0 {
                4
            } else if b >= 0xE0 {
                3
            } else {
                2
            };
            return if back < need { &buf[..i] } else { buf };
        }
        // Continuation byte (0x80..=0xBF): keep looking back for the lead.
    }
    buf
}

// ── Input ─────────────────────────────────────────────────────────────────────

/// Tool input plus whether the byte limit cut it short.
///
/// Returned by the `*_checked` readers. A tool whose result is a claim about
/// the *whole* input — a summary, a count, a search — must thread `truncated`
/// into its `Output` so `--json` reports it: a stderr warning is invisible to
/// anything parsing stdout, which makes a partial answer look complete.
///
/// Tools that merely transform or generate (the intent string for `lxsh`, the
/// bullet points for `lxdraft`) do not need this — the stderr warning is the
/// right and sufficient treatment, and they use the plain readers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    /// The text that was read, already truncated to the limit if it was hit.
    pub text: String,
    /// True when the source held more bytes than the limit allowed.
    pub truncated: bool,
}

impl Input {
    /// Discard the flag and keep the text.
    pub fn into_text(self) -> String {
        self.text
    }
}

impl std::ops::Deref for Input {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

/// Shared tail of every read: drop a split character, warn, and wrap.
fn finish_read(buf: Vec<u8>, truncated: bool, max_bytes: usize, label: &str) -> Input {
    // Only trim on a truncating read — on a complete read, malformed bytes are
    // the user's real data and must stay visible as replacement characters.
    let bytes = if truncated {
        trim_incomplete_utf8_tail(&buf)
    } else {
        &buf[..]
    };
    if truncated {
        // Report bytes below 1 KiB — integer division would otherwise render a
        // small `--max-input-bytes` as a nonsensical "0 KiB".
        let size = if max_bytes >= 1024 {
            format!("{} KiB", max_bytes / 1024)
        } else {
            format!("{max_bytes} bytes")
        };
        crate::output::warn(&format!(
            "{label} truncated at {size} — results may be incomplete; \
             raise --max-input-bytes to see more"
        ));
    }
    Input {
        text: String::from_utf8_lossy(bytes).into_owned(),
        truncated,
    }
}

// ── Stdin reading ─────────────────────────────────────────────────────────────

/// Read all of stdin up to `max_bytes`.
///
/// - Fails immediately with `LxError::BadUsage` when stdin is a TTY (interactive
///   use without piped input).
/// - Blocks until EOF for piped/redirected stdin — no timeout, matching the
///   behaviour of jq, ripgrep, and every standard Unix filter.
/// - On size overflow: truncates at `max_bytes`, emits a warning on stderr,
///   continues.
///
/// Use [`read_stdin_checked`] when the tool must report truncation in `--json`.
pub fn read_stdin(max_bytes: usize) -> Result<String, LxError> {
    read_stdin_checked(max_bytes).map(Input::into_text)
}

/// Like [`read_stdin`], but also reports whether the limit cut the input short.
pub fn read_stdin_checked(max_bytes: usize) -> Result<Input, LxError> {
    if crate::platform::is_tty(crate::platform::Fd::Stdin) {
        return Err(LxError::BadUsage(
            "no input provided — pipe data into this tool or use --file".to_string(),
        ));
    }

    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut buf = Vec::with_capacity(max_bytes.min(65_536));
    let mut chunk = [0u8; 8_192];
    let mut total = 0usize;
    let mut truncated = false;

    loop {
        match handle.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let remaining = max_bytes.saturating_sub(total);
                if n > remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                total += n;
                if total == max_bytes {
                    // The budget is exactly full, which is not truncation on its
                    // own — the input may be exactly `max_bytes` long. Probe one
                    // byte to tell "exactly at the limit" from "longer".
                    let mut probe = [0u8; 1];
                    match handle.read(&mut probe) {
                        Ok(0) => {}
                        Ok(_) => truncated = true,
                        Err(e) => return Err(LxError::BadUsage(format!("stdin read error: {e}"))),
                    }
                    break;
                }
            }
            Err(e) => return Err(LxError::BadUsage(format!("stdin read error: {e}"))),
        }
    }

    Ok(finish_read(buf, truncated, max_bytes, "input"))
}

// ── File reading ──────────────────────────────────────────────────────────────

/// Read a file limited to `max_bytes`, truncating with a warning if exceeded.
///
/// If `allowed_root` is `Some(root)`, the resolved path must be inside `root`
/// (fsbound principle). Symlinks that escape the root are rejected with
/// `LxError::SecurityAbort`.
/// Use [`read_file_checked`] when the tool must report truncation in `--json`.
pub fn read_file(
    path: &Path,
    max_bytes: usize,
    allowed_root: Option<&Path>,
) -> Result<String, LxError> {
    read_file_checked(path, max_bytes, allowed_root).map(Input::into_text)
}

/// Like [`read_file`], but also reports whether the limit cut the file short.
pub fn read_file_checked(
    path: &Path,
    max_bytes: usize,
    allowed_root: Option<&Path>,
) -> Result<Input, LxError> {
    // Resolve the path to catch symlink escapes.
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| LxError::BadUsage(format!("cannot resolve {}: {e}", path.display())))?;

    if let Some(root) = allowed_root {
        let root_canonical = std::fs::canonicalize(root).map_err(|e| {
            LxError::BadUsage(format!("cannot resolve root {}: {e}", root.display()))
        })?;
        if !canonical.starts_with(&root_canonical) {
            return Err(LxError::SecurityAbort(format!(
                "path {} escapes allowed root {}",
                canonical.display(),
                root_canonical.display()
            )));
        }
    }

    read_file_raw(&canonical, max_bytes)
}

/// Read a file with no fsbound check.
pub fn read_file_limited(path: &Path, max_bytes: usize) -> Result<String, LxError> {
    read_file(path, max_bytes, None)
}

/// Like [`read_file_limited`], but also reports truncation.
pub fn read_file_limited_checked(path: &Path, max_bytes: usize) -> Result<Input, LxError> {
    read_file_checked(path, max_bytes, None)
}

fn read_file_raw(path: &Path, max_bytes: usize) -> Result<Input, LxError> {
    use std::io::BufReader;

    let file = std::fs::File::open(path)
        .map_err(|e| LxError::BadUsage(format!("cannot open {}: {e}", path.display())))?;
    let mut reader = BufReader::new(file);
    let mut buf = Vec::with_capacity(max_bytes.min(65_536));
    let mut chunk = [0u8; 8_192];
    let mut total = 0usize;
    let mut truncated = false;

    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let remaining = max_bytes.saturating_sub(total);
                if n > remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                total += n;
                if total == max_bytes {
                    // Budget exactly full — probe one byte to distinguish a file
                    // that is exactly `max_bytes` long from one that is longer.
                    let mut probe = [0u8; 1];
                    match reader.read(&mut probe) {
                        Ok(0) => {}
                        Ok(_) => truncated = true,
                        Err(e) => return Err(LxError::BadUsage(format!("read error: {e}"))),
                    }
                    break;
                }
            }
            Err(e) => return Err(LxError::BadUsage(format!("read error: {e}"))),
        }
    }

    Ok(finish_read(buf, truncated, max_bytes, "file"))
}

// ── Atomic file write ─────────────────────────────────────────────────────────

/// Atomically write `content` to `path`.
///
/// Writes to a temp file in the same directory, then renames over the target.
/// Concurrent readers see either the old version or the new version — never a
/// partial write. On error the temp file is cleaned up.
///
/// Rename is atomic on POSIX. On Windows it is best-effort (the file is fully
/// written before rename, which is still safer than a direct overwrite).
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<(), LxError> {
    let parent = path.parent().ok_or_else(|| {
        LxError::BadUsage(format!(
            "cannot determine parent directory of {}",
            path.display()
        ))
    })?;

    let mut tmp = TempFile::create_in(parent)?;

    {
        let f = tmp.file.as_mut().expect("file is Some after creation");
        f.write_all(content)
            .map_err(|e| LxError::BadUsage(format!("write to temp file failed: {e}")))?;
        f.flush()
            .map_err(|e| LxError::BadUsage(format!("flush failed: {e}")))?;
    }
    // Close before rename — required on Windows (open handles block rename).
    drop(tmp.file.take());

    std::fs::rename(&tmp.path, path).map_err(|e| {
        LxError::BadUsage(format!(
            "atomic rename {} -> {} failed: {e}",
            tmp.path.display(),
            path.display()
        ))
    })?;

    tmp.disarmed = true;
    Ok(())
}

struct TempFile {
    file: Option<std::fs::File>,
    path: PathBuf,
    disarmed: bool,
}

impl TempFile {
    fn create_in(dir: &Path) -> Result<Self, LxError> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let pid = std::process::id();
        let name = format!(".lx_tmp_{pid}_{nanos:08x}");
        let path = dir.join(name);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                LxError::BadUsage(format!("cannot create temp file {}: {e}", path.display()))
            })?;
        Ok(TempFile {
            file: Some(file),
            path,
            disarmed: false,
        })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.disarmed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// ── Input resolution ─────────────────────────────────────────────────────────

/// Resolve tool input with this priority:
/// 1. `--file <path>` if given — reads and returns the file contents
/// 2. stdin if not a TTY (piped)
/// 3. Error with a helpful hint if stdin is a TTY and no `--file` was given
///
/// Use [`resolve_input_checked`] when the tool must report truncation in
/// `--json` — see [`Input`] for which tools that is.
pub fn resolve_input(file: Option<&std::path::Path>, max_bytes: usize) -> Result<String, LxError> {
    resolve_input_checked(file, max_bytes).map(Input::into_text)
}

/// Like [`resolve_input`], but also reports whether the limit cut the input short.
pub fn resolve_input_checked(
    file: Option<&std::path::Path>,
    max_bytes: usize,
) -> Result<Input, LxError> {
    if let Some(path) = file {
        return read_file_checked(path, max_bytes, None);
    }
    read_stdin_checked(max_bytes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── truncate_at_char_boundary ──

    #[test]
    fn truncate_at_char_boundary_leaves_short_input_alone() {
        assert_eq!(truncate_at_char_boundary("hello", 1024), "hello");
        assert_eq!(truncate_at_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn truncate_at_char_boundary_snaps_back_off_a_split_character() {
        // 'ä' is two bytes: cutting at 2 lands inside it. A bare `&s[..2]`
        // panics here — this is the regression the helper exists to prevent.
        assert_eq!(truncate_at_char_boundary("aä", 2), "a");
        // A four-byte character cut anywhere inside it drops the whole char.
        assert_eq!(truncate_at_char_boundary("🦀", 1), "");
        assert_eq!(truncate_at_char_boundary("🦀", 3), "");
        assert_eq!(truncate_at_char_boundary("🦀", 4), "🦀");
    }

    #[test]
    fn truncate_at_char_boundary_never_panics_at_any_offset() {
        let s = "aä🦀e\u{00e9}f";
        for n in 0..=s.len() {
            let t = truncate_at_char_boundary(s, n);
            assert!(s.starts_with(t), "result must be a prefix of the input");
            assert!(t.len() <= n, "result must not exceed the requested cap");
        }
    }

    // ── trim_incomplete_utf8_tail ──

    #[test]
    fn trim_incomplete_utf8_tail_drops_a_split_character() {
        // 'ä' is two bytes; cut after its lead byte leaves a dangling fragment.
        assert_eq!(trim_incomplete_utf8_tail(&[b'a', 0xC3]), b"a");
        // A complete 'ä' survives untouched.
        assert_eq!(
            trim_incomplete_utf8_tail(&[b'a', 0xC3, 0xA4]),
            &[b'a', 0xC3, 0xA4]
        );
        // A four-byte emoji missing its final continuation byte.
        assert_eq!(trim_incomplete_utf8_tail(&[0xF0, 0x9F, 0xA6]), &[] as &[u8]);
        // ASCII and empty input are never touched.
        assert_eq!(trim_incomplete_utf8_tail(b"hello"), b"hello");
        assert_eq!(trim_incomplete_utf8_tail(&[]), &[] as &[u8]);
    }

    #[test]
    fn truncated_read_does_not_end_in_a_replacement_character() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_split_char.txt");
        // The "x" prefix puts every 'ä' on an odd byte offset, so an even cap
        // is guaranteed to land inside one.
        std::fs::write(&path, format!("x{}", "ä".repeat(100)).as_bytes()).unwrap();
        let input = read_file_checked(&path, 10, None).unwrap();
        assert!(input.truncated);
        assert!(
            !input.text.ends_with('\u{FFFD}'),
            "a split character must be dropped, not become U+FFFD: {:?}",
            input.text
        );
        std::fs::remove_file(&path).ok();
    }

    // ── truncation flag ──

    #[test]
    fn input_exactly_at_the_limit_is_not_truncated() {
        // The old `n >= remaining` test reported truncation here even though
        // nothing was dropped — and lxjson's byte-length heuristic rejected it.
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_exact.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let input = read_file_checked(&path, 11, None).unwrap();
        assert_eq!(&*input, "hello world");
        assert!(!input.truncated, "input of exactly max_bytes is complete");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn input_one_byte_over_the_limit_is_truncated() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_over.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let input = read_file_checked(&path, 10, None).unwrap();
        assert_eq!(&*input, "hello worl");
        assert!(input.truncated);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn plain_and_checked_readers_return_the_same_text() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_parity.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let plain = read_file(&path, 6, None).unwrap();
        let checked = read_file_checked(&path, 6, None).unwrap();
        assert_eq!(plain, checked.text);
        assert!(checked.truncated);
        std::fs::remove_file(&path).ok();
    }

    // ── read_file ──

    #[test]
    fn read_file_within_limit() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_read.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let content = read_file(&path, 1024, None).unwrap();
        assert_eq!(content, "hello world");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_file_truncates_at_limit() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_io_truncate.txt");
        std::fs::write(&path, vec![b'x'; 100]).unwrap();
        let content = read_file(&path, 10, None).unwrap();
        assert_eq!(content.len(), 10);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_file_missing_returns_error() {
        let result = read_file(Path::new("/nonexistent/xyz.txt"), 1024, None);
        assert!(result.is_err());
    }

    #[test]
    fn read_file_fsbound_rejects_escape() {
        // Create a real temp file and a root that doesn't contain it.
        let tmp = std::env::temp_dir();
        let file = tmp.join("lx_fsbound_test.txt");
        std::fs::write(&file, b"data").unwrap();

        // Use a root that is a subdirectory of tmp → file is outside it.
        let root = tmp.join("lx_fsbound_root_dir");
        std::fs::create_dir_all(&root).unwrap();

        let result = read_file(&file, 1024, Some(&root));
        assert!(
            matches!(result, Err(LxError::SecurityAbort(_))),
            "expected SecurityAbort, got {result:?}"
        );
        std::fs::remove_file(&file).ok();
        std::fs::remove_dir(&root).ok();
    }

    // ── write_atomic ──

    #[test]
    fn write_atomic_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_atomic_write.txt");
        write_atomic(&path, b"hello atomic").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello atomic");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_atomic_overwrite.txt");
        std::fs::write(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        std::fs::remove_file(&path).ok();
    }

    // ── resolve_input ──

    #[test]
    fn resolve_input_reads_file_when_given() {
        let dir = std::env::temp_dir();
        let path = dir.join("lx_core_resolve_input.txt");
        std::fs::write(&path, b"resolve test").unwrap();
        let content = resolve_input(Some(&path), 1024).unwrap();
        assert_eq!(content, "resolve test");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_input_missing_file_returns_error() {
        let result = resolve_input(Some(std::path::Path::new("/nonexistent/missing.txt")), 1024);
        assert!(result.is_err());
    }

    #[test]
    fn write_atomic_cleans_up_on_error() {
        // Use a UNIQUE per-test subdirectory, not the shared temp root: other
        // tools' tests run concurrently and create their own `.lx_tmp_*` files in
        // the global temp dir, which would make a scan of that dir flaky.
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("lx_core_cleanup_test_{pid}_{nanos:x}"));
        std::fs::create_dir(&base).unwrap();

        // Make the destination an existing DIRECTORY. The temp file is created
        // successfully in `base` (parent exists), then the final rename fails
        // (cannot rename a file over a directory) — exercising the real
        // cleanup-on-error path: TempFile::drop removes the temp file.
        let dest = base.join("dest_is_a_dir");
        std::fs::create_dir(&dest).unwrap();
        let result = write_atomic(&dest, b"data");
        assert!(result.is_err(), "rename over a directory should fail");

        // Only our own isolated dir is scanned, so concurrent tests can't interfere.
        let leftovers: Vec<_> = std::fs::read_dir(&base)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".lx_tmp_"))
            .collect();
        let leftover_names: Vec<_> = leftovers.iter().map(|e| e.file_name()).collect();
        std::fs::remove_dir_all(&base).ok();
        assert!(
            leftovers.is_empty(),
            "leftover temp files: {leftover_names:?}"
        );
    }
}
