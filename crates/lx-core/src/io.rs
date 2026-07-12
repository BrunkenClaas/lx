#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::exit::LxError;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default maximum stdin bytes read before truncation (512 KiB).
pub const DEFAULT_MAX_INPUT_BYTES: usize = 512 * 1024;

// ── Stdin reading ─────────────────────────────────────────────────────────────

/// Read all of stdin up to `max_bytes`.
///
/// - Fails immediately with `LxError::BadUsage` when stdin is a TTY (interactive
///   use without piped input).
/// - Blocks until EOF for piped/redirected stdin — no timeout, matching the
///   behaviour of jq, ripgrep, and every standard Unix filter.
/// - On size overflow: truncates at `max_bytes`, emits a warning on stderr,
///   continues.
pub fn read_stdin(max_bytes: usize) -> Result<String, LxError> {
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
                if n >= remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    break;
                } else {
                    buf.extend_from_slice(&chunk[..n]);
                    total += n;
                }
            }
            Err(e) => return Err(LxError::BadUsage(format!("stdin read error: {e}"))),
        }
    }

    if truncated {
        crate::output::warn(&format!("input truncated at {} KiB", max_bytes / 1024));
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}

// Backward-compat alias kept for existing tool code.
pub fn read_stdin_limited(max_bytes: usize) -> Result<String, LxError> {
    read_stdin(max_bytes)
}

// ── File reading ──────────────────────────────────────────────────────────────

/// Read a file limited to `max_bytes`, truncating with a warning if exceeded.
///
/// If `allowed_root` is `Some(root)`, the resolved path must be inside `root`
/// (fsbound principle). Symlinks that escape the root are rejected with
/// `LxError::SecurityAbort`.
pub fn read_file(
    path: &Path,
    max_bytes: usize,
    allowed_root: Option<&Path>,
) -> Result<String, LxError> {
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

/// Backward-compat alias — no fsbound check.
pub fn read_file_limited(path: &Path, max_bytes: usize) -> Result<String, LxError> {
    read_file(path, max_bytes, None)
}

fn read_file_raw(path: &Path, max_bytes: usize) -> Result<String, LxError> {
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
                if n >= remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    break;
                } else {
                    buf.extend_from_slice(&chunk[..n]);
                    total += n;
                }
            }
            Err(e) => return Err(LxError::BadUsage(format!("read error: {e}"))),
        }
    }

    if truncated {
        crate::output::warn(&format!("file truncated at {} KiB", max_bytes / 1024));
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
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
pub fn resolve_input(file: Option<&std::path::Path>, max_bytes: usize) -> Result<String, LxError> {
    if let Some(path) = file {
        return read_file(path, max_bytes, None);
    }
    read_stdin(max_bytes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
